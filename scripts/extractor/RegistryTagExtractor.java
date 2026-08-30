// Dumps the NBT tag each field of a synced registry entry carries.
//
// The registry payload a client is sent is NBT, and it is built here from json. Json has one number
// type and NBT has six, so every integer would go out as a Long and every real as a Double. A
// lenient client coerces them; a strict one reads the payload into typed structs and refuses a
// field whose tag is not what its schema says.
//
// There is no rule to guess by — most numeric fields in these registries are Float, some are Int,
// and they sit in types declared far from the registry itself. So the game is asked: each entry is
// read through its own codec and written back out as NBT, and the tag of every field is recorded.

import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.TreeSet;

import com.google.gson.JsonElement;
import com.google.gson.JsonParser;
import com.mojang.serialization.Codec;
import com.mojang.serialization.DataResult;
import com.mojang.serialization.DynamicOps;
import com.mojang.serialization.JsonOps;

import java.util.concurrent.ForkJoinPool;
import java.util.stream.Stream;

import net.minecraft.core.HolderLookup;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.server.RegistryLayer;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagLoader;
import net.minecraft.resources.RegistryOps;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;

public final class RegistryTagExtractor {
    /// Every field path seen, and the tag ids written at it. A name alone is not enough: an
    /// enchantment writes `base` as an `Int` in one place and a `Float` in another, so what is
    /// recorded is where the field sits, not what it is called.
    private static final Map<String, Map<String, TreeSet<Integer>>> TAGS = new TreeMap<>();
    private static final List<String> UNREAD = new ArrayList<>();

    public static void main(final String[] args) throws Exception {
        // The version has to be worked out before anything else is set up, as the server does.
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        Path payload = Path.of(args[0]);
        Path out = Path.of(args[1]);

        // The registries alone are not enough. An enchantment names item tags, a dimension type
        // names a world clock, and a codec that cannot resolve one refuses the whole entry — so
        // everything the world would load is loaded, exactly the way the server loads it.
        PackRepository packs = ServerPacksSource.createVanillaTrustedRepository();
        packs.reload();
        // A fresh repository has nothing selected, and an unselected pack is not opened.
        packs.setSelected(packs.getAvailableIds());

        RegistryAccess.Frozen everything;
        try (MultiPackResourceManager resources =
                new MultiPackResourceManager(PackType.SERVER_DATA, packs.openAllSelected())) {
            LayeredRegistryAccess<RegistryLayer> layers = RegistryLayer.createRegistryAccess();
            List<Registry.PendingTags<?>> staticTags =
                TagLoader.loadTagsForExistingRegistries(resources, layers.getLayer(RegistryLayer.STATIC));
            // Binding them puts the tags on the registries themselves, so an access built from the
            // layers afterwards knows them too. Looking them up through fresh lookups is not the
            // same thing: writing a holder checks the set it belongs to.
            staticTags.forEach(Registry.PendingTags::apply);
            List<HolderLookup.RegistryLookup<?>> context = TagLoader.buildUpdatedLookups(
                layers.getAccessForLoading(RegistryLayer.WORLDGEN), staticTags);
            RegistryAccess.Frozen loaded = RegistryDataLoader.load(
                resources, context, RegistryDataLoader.WORLDGEN_REGISTRIES, ForkJoinPool.commonPool()
            ).join();
            // One access holding both layers, rather than a wrapper around two: writing a holder
            // checks that the registry set it came from is the one being written into, and a
            // wrapper is not that set.
            everything = layers.replaceFrom(RegistryLayer.WORLDGEN, loaded).compositeAccess();
        }
        DynamicOps<JsonElement> fromJson = RegistryOps.create(JsonOps.INSTANCE, everything);
        DynamicOps<Tag> toNbt = RegistryOps.create(NbtOps.INSTANCE, everything);

        JsonElement root = JsonParser.parseString(Files.readString(payload, StandardCharsets.UTF_8));
        for (Map.Entry<String, JsonElement> registry : root.getAsJsonObject().entrySet()) {
            String name = registry.getKey();
            Codec<?> codec = codecFor(name);
            if (codec == null) {
                UNREAD.add(name + " (not a synced registry)");
                continue;
            }
            for (Map.Entry<String, JsonElement> entry : registry.getValue().getAsJsonObject().entrySet()) {
                record(name, entry.getKey(), codec, fromJson, toNbt, entry.getValue());
            }
        }

        try (PrintWriter writer = new PrintWriter(Files.newBufferedWriter(out, StandardCharsets.UTF_8))) {
            writer.println("{");
            writer.println("  \"comment\": \"field name -> nbt tag id, per registry. Asked of the game.\",");
            writer.print("  \"unread\": [");
            for (int i = 0; i < UNREAD.size(); i++) {
                writer.print((i > 0 ? ", " : "") + quote(UNREAD.get(i)));
            }
            writer.println("],");
            writer.println("  \"registries\": {");
            int registriesLeft = TAGS.size();
            for (Map.Entry<String, Map<String, TreeSet<Integer>>> registry : TAGS.entrySet()) {
                writer.println("    \"" + registry.getKey() + "\": {");
                int fieldsLeft = registry.getValue().size();
                for (Map.Entry<String, TreeSet<Integer>> field : registry.getValue().entrySet()) {
                    StringBuilder tags = new StringBuilder();
                    for (int tag : field.getValue()) {
                        tags.append(tags.length() == 0 ? "" : ", ").append(tag);
                    }
                    writer.println("      \"" + field.getKey() + "\": [" + tags + "]"
                        + (--fieldsLeft > 0 ? "," : ""));
                }
                writer.println("    }" + (--registriesLeft > 0 ? "," : ""));
            }
            writer.println("  }");
            writer.println("}");
        }
    }

    /// A json string, with what json needs escaping escaped.
    private static String quote(final String text) {
        StringBuilder out = new StringBuilder("\"");
        for (int i = 0; i < text.length(); i++) {
            char c = text.charAt(i);
            switch (c) {
                case '"' -> out.append("\\\"");
                case '\\' -> out.append("\\\\");
                case '\n' -> out.append("\\n");
                case '\r' -> out.append("\\r");
                case '\t' -> out.append("\\t");
                default -> {
                    if (c < 0x20) {
                        out.append(String.format("\\u%04x", (int) c));
                    } else {
                        out.append(c);
                    }
                }
            }
        }
        return out.append('"').toString();
    }

    /// The codec the server sends this registry's entries with.
    private static Codec<?> codecFor(final String registry) {
        for (RegistryDataLoader.RegistryData<?> data : RegistryDataLoader.SYNCHRONIZED_REGISTRIES) {
            if (data.key().identifier().toString().equals(registry)) {
                return data.elementCodec();
            }
        }
        return null;
    }

    /// Reads one entry through its codec and writes it back as NBT, noting every field's tag.
    private static <T> void record(
        final String registry,
        final String entry,
        final Codec<T> codec,
        final DynamicOps<JsonElement> fromJson,
        final DynamicOps<Tag> toNbt,
        final JsonElement json
    ) {
        DataResult<T> parsed = codec.parse(fromJson, json);
        if (parsed.isError()) {
            UNREAD.add(registry + "/" + entry + ": " + parsed.error().get().message());
            return;
        }
        DataResult<Tag> written = codec.encodeStart(toNbt, parsed.getOrThrow());
        if (written.isError()) {
            UNREAD.add(registry + "/" + entry + " (writing): " + written.error().get().message());
            return;
        }
        walk(registry, written.getOrThrow(), "");
    }

    /// Notes the tag of every field by where it sits, however deep. A list contributes one step to
    /// the path however long it is, since every element of one is the same shape.
    private static void walk(final String registry, final Tag tag, final String path) {
        if (tag instanceof CompoundTag compound) {
            for (String key : compound.keySet()) {
                Tag value = compound.get(key);
                if (value == null) {
                    continue;
                }
                String below = path + "/" + key;
                TAGS.computeIfAbsent(registry, r -> new TreeMap<>())
                    .computeIfAbsent(below, k -> new TreeSet<>())
                    .add((int) value.getId());
                walk(registry, value, below);
            }
        } else if (tag instanceof ListTag list) {
            for (Tag element : list) {
                walk(registry, element, path + "[]");
            }
        }
    }
}
