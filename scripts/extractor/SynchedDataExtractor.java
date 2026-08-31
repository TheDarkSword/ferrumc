// Dumps the indexed key-value store a client reads an entity's state out of.
//
// Two things here have to come from the game rather than from a wiki page. The first is the
// serializer table: a field's type is written on the wire as the serializer's registration number,
// and that number is registration order, which moves between versions. The second is the index
// layout, which vanilla allocates by walking the entity class tree, so a field's index depends on
// every class above it. Both are silently catastrophic when wrong -- the client reads the bytes as
// whatever type it expected there and renders nonsense -- so neither is transcribed.
//
// Entities are built with an uninitialised level, which is enough because the only thing the base
// constructor asks a level for is the next entity id, and that is a constant.
//
// Since 26.1 the server jar ships with its own names, so this compiles straight against it.

import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ForkJoinPool;

import net.minecraft.SharedConstants;
import io.netty.buffer.Unpooled;

import net.minecraft.core.HolderLookup;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.component.DataComponentInitializers;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.syncher.EntityDataAccessor;
import net.minecraft.network.syncher.EntityDataSerializer;
import net.minecraft.network.syncher.EntityDataSerializers;
import net.minecraft.network.syncher.SynchedEntityData;
import net.minecraft.resources.Identifier;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagLoader;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.MinecraftServer;
import net.minecraft.util.debug.ServerDebugSubscribers;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.core.Direction;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.HumanoidArm;
import net.minecraft.world.entity.Pose;
import net.minecraft.world.entity.animal.armadillo.Armadillo;
import net.minecraft.world.entity.animal.golem.CopperGolemState;
import net.minecraft.world.entity.animal.sniffer.Sniffer;
import net.minecraft.world.level.block.WeatheringCopper;
import net.minecraft.world.flag.FeatureFlagSet;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.level.storage.LevelData;
import net.minecraft.world.level.storage.PrimaryLevelData;
import net.minecraft.server.ServerScoreboard;
import net.minecraft.world.level.Level;
import sun.misc.Unsafe;

public final class SynchedDataExtractor {
    public static void main(final String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        Map<EntityDataSerializer<?>, String> serializerNames = serializerNames();
        RegistryAccess registries = loadEverything();
        Level level = hollowLevel(registries);

        try (PrintWriter out = new PrintWriter(
                Files.newBufferedWriter(Path.of(args[0]), StandardCharsets.UTF_8))) {
            out.println("{");
            out.println("  \"version\": \"" + SharedConstants.getCurrentVersion().name() + "\",");

            // The wire type tag, which is the order the serializers were registered in.
            out.println("  \"serializers\": [");
            List<String> serializers = registeredSerializers(serializerNames);
            for (int i = 0; i < serializers.size(); i++) {
                out.println("    \"" + serializers.get(i) + "\""
                    + (i + 1 < serializers.size() ? "," : ""));
            }
            out.println("  ],");

            // The small vocabularies a field can hold, so nothing downstream has to name a
            // number. Each value is written by its own codec and read back, so the numbers are the
            // ones a client reads rather than the order the constants happen to be declared in.
            out.println("  \"enums\": {");
            out.print(namedValues("pose", Pose.values(), EntityDataSerializers.POSE, registries));
            out.println(",");
            out.print(namedValues("direction", Direction.values(), EntityDataSerializers.DIRECTION, registries));
            out.println(",");
            out.print(namedValues("humanoid_arm", HumanoidArm.values(), EntityDataSerializers.HUMANOID_ARM, registries));
            out.println(",");
            out.print(namedValues("sniffer_state", Sniffer.State.values(), EntityDataSerializers.SNIFFER_STATE, registries));
            out.println(",");
            out.print(namedValues("armadillo_state", Armadillo.ArmadilloState.values(), EntityDataSerializers.ARMADILLO_STATE, registries));
            out.println(",");
            out.print(namedValues("copper_golem_state", CopperGolemState.values(), EntityDataSerializers.COPPER_GOLEM_STATE, registries));
            out.println(",");
            out.print(namedValues("weathering_copper_state", WeatheringCopper.WeatherState.values(), EntityDataSerializers.WEATHERING_COPPER_STATE, registries));
            out.println();
            out.println("  },");

            out.println("  \"types\": {");
            List<EntityType<?>> types = BuiltInRegistries.ENTITY_TYPE.stream().toList();
            List<String> entries = new ArrayList<>();
            List<String> failures = new ArrayList<>();
            for (EntityType<?> type : types) {
                Identifier name = BuiltInRegistries.ENTITY_TYPE.getKey(type);
                try {
                    entries.add(layout(type, name, level, serializerNames, registries));
                } catch (Throwable failure) {
                    failures.add(name + ": " + failure);
                    if (System.getenv("TRACE") != null) {
                        failure.printStackTrace();
                    }
                }
            }
            for (int i = 0; i < entries.size(); i++) {
                out.println(entries.get(i) + (i + 1 < entries.size() ? "," : ""));
            }
            out.println("  }");
            out.println("}");

            for (String failure : failures) {
                System.err.println("could not build " + failure);
            }
            System.err.println(entries.size() + " of " + types.size() + " types laid out");
        }
    }

    /// One small vocabulary, as name to the number a client reads.
    private static <T> String namedValues(
        final String name,
        final T[] values,
        final EntityDataSerializer<T> serializer,
        final RegistryAccess registries
    ) {
        StringBuilder text = new StringBuilder("    \"" + name + "\": {");
        for (int i = 0; i < values.length; i++) {
            RegistryFriendlyByteBuf buffer = new RegistryFriendlyByteBuf(Unpooled.buffer(), registries);
            serializer.codec().encode(buffer, values[i]);
            int id = net.minecraft.network.VarInt.read(buffer);
            text.append("\"").append(values[i].toString().toLowerCase()).append("\": ").append(id);
            if (i + 1 < values.length) {
                text.append(", ");
            }
        }
        return text.append("}").toString();
    }

    /// One type's fields, in index order, with the class each was declared on.
    private static String layout(
        final EntityType<?> type,
        final Identifier name,
        final Level level,
        final Map<EntityDataSerializer<?>, String> serializerNames,
        final RegistryAccess registries
    ) throws Exception {
        Entity entity = name.getPath().equals("player")
            ? new HollowPlayer(level)
            : type.create(level, EntitySpawnReason.COMMAND);
        if (entity == null) {
            throw new IllegalStateException("no factory result");
        }
        Map<Integer, String> owners = accessorOwners(entity.getClass());

        Field itemsById = SynchedEntityData.class.getDeclaredField("itemsById");
        itemsById.setAccessible(true);
        Object[] items = (Object[]) itemsById.get(entity.getEntityData());

        Field accessorField = SynchedEntityData.DataItem.class.getDeclaredField("accessor");
        Field initialField = SynchedEntityData.DataItem.class.getDeclaredField("initialValue");
        accessorField.setAccessible(true);
        initialField.setAccessible(true);

        StringBuilder text = new StringBuilder();
        text.append("    \"").append(name).append("\": {\n");
        text.append("      \"class\": \"").append(entity.getClass().getName()).append("\",\n");
        text.append("      \"fields\": [\n");
        for (int i = 0; i < items.length; i++) {
            EntityDataAccessor<?> accessor = (EntityDataAccessor<?>) accessorField.get(items[i]);
            Object initial = initialField.get(items[i]);
            text.append("        {\"index\": ").append(accessor.id())
                .append(", \"serializer\": \"").append(serializerNames.get(accessor.serializer()))
                .append("\", \"owner\": \"").append(owners.getOrDefault(accessor.id(), "?"))
                .append("\", \"name\": \"").append(fieldName(owners, accessor.id()))
                .append("\", \"default_text\": ").append(json(initial))
                .append(", \"default\": \"").append(wireForm(accessor, initial, registries))
                .append("\"}").append(i + 1 < items.length ? "," : "").append("\n");
        }
        text.append("      ]\n");
        text.append("    }");
        return text.toString();
    }

    /// Which class declared the accessor at each index, and under what name.
    ///
    /// Vanilla numbers a class's fields from one past its superclass's last, so walking the chain
    /// from the top down and reading the static accessors off each class recovers the whole layout
    /// and says where every index came from.
    private static Map<Integer, String> accessorOwners(final Class<?> leaf) throws Exception {
        Map<Integer, String> owners = new HashMap<>();
        for (Class<?> clazz = leaf; clazz != null && clazz != Object.class; clazz = clazz.getSuperclass()) {
            for (Field field : clazz.getDeclaredFields()) {
                if (!EntityDataAccessor.class.isAssignableFrom(field.getType())) {
                    continue;
                }
                field.setAccessible(true);
                EntityDataAccessor<?> accessor = (EntityDataAccessor<?>) field.get(null);
                if (accessor != null) {
                    owners.put(accessor.id(), clazz.getSimpleName() + "#" + field.getName());
                }
            }
        }
        return owners;
    }

    private static String fieldName(final Map<Integer, String> owners, final int index) {
        String owner = owners.get(index);
        return owner == null ? "?" : owner.substring(owner.indexOf('#') + 1);
    }

    /// The serializers by the name they carry as a field, so the dump reads as the game reads.
    private static Map<EntityDataSerializer<?>, String> serializerNames() throws Exception {
        Map<EntityDataSerializer<?>, String> names = new IdentityHashMap<>();
        for (Field field : EntityDataSerializers.class.getDeclaredFields()) {
            if (!EntityDataSerializer.class.isAssignableFrom(field.getType())) {
                continue;
            }
            field.setAccessible(true);
            Object value = field.get(null);
            if (value != null) {
                names.put((EntityDataSerializer<?>) value, field.getName().toLowerCase());
            }
        }
        return names;
    }

    /// The registration order, which is what goes on the wire as a field's type.
    private static List<String> registeredSerializers(
        final Map<EntityDataSerializer<?>, String> names
    ) {
        Map<Integer, String> byId = new HashMap<>();
        for (Map.Entry<EntityDataSerializer<?>, String> entry : names.entrySet()) {
            int id = EntityDataSerializers.getSerializedId(entry.getKey());
            if (id >= 0) {
                byId.put(id, entry.getValue());
            }
        }
        List<String> ordered = new ArrayList<>();
        for (int id = 0; id < byId.size(); id++) {
            String name = byId.get(id);
            if (name == null) {
                throw new IllegalStateException("serializer " + id + " has no name");
            }
            ordered.add(name);
        }
        return ordered;
    }

    /// A default value as bytes, written the way the game writes it down a connection.
    ///
    /// Reading a value's type off its class means guessing how the game numbers it. Asking its own
    /// codec to write it does not: what comes back is what a client would have read.
    @SuppressWarnings("unchecked")
    private static <T> String wireForm(
        final EntityDataAccessor<T> accessor, final Object value, final RegistryAccess registries
    ) {
        RegistryFriendlyByteBuf buffer =
            new RegistryFriendlyByteBuf(Unpooled.buffer(), registries);
        try {
            ((StreamCodec<RegistryFriendlyByteBuf, T>) accessor.serializer().codec())
                .encode(buffer, (T) value);
        } catch (RuntimeException unwritable) {
            return "";
        }
        StringBuilder hex = new StringBuilder();
        while (buffer.isReadable()) {
            hex.append(String.format("%02x", buffer.readByte() & 0xFF));
        }
        return hex.toString();
    }

    /// A default value, as far as it can be written down.
    ///
    /// Most are numbers, flags or empty optionals; the rest are printed as the game prints them,
    /// which is enough to check a layout by eye.
    private static String json(final Object value) {
        if (value == null) {
            return "null";
        }
        if (value instanceof Boolean || value instanceof Number) {
            return value.toString();
        }
        if (value instanceof java.util.Optional<?> optional) {
            return optional.isPresent() ? json(optional.get()) : "null";
        }
        if (value instanceof java.util.OptionalInt optional) {
            return optional.isPresent() ? Integer.toString(optional.getAsInt()) : "null";
        }
        return "\"" + value.toString().replace("\\", "\\\\").replace("\"", "\\\"") + "\"";
    }

    /// A level that exists only enough to be passed to a constructor.
    ///
    /// Entity constructors ask a level for very little: the next entity id, the registries, and a
    /// server to hang debug listeners off. Those are answered here and the rest of the level is
    /// never initialised, so anything reaching past them throws and its type is reported unread.
    private static Level hollowLevel(final RegistryAccess registries) throws Exception {
        MinecraftServer server = hollow(net.minecraft.server.dedicated.DedicatedServer.class);
        poke(server, "debugSubscribers", hollow(ServerDebugSubscribers.class));

        HollowLevel level = hollow(HollowLevel.class);
        poke(level, "registries", registries);
        poke(level, "hollowServer", server);
        poke(level, "hollowLevelData", hollow(PrimaryLevelData.class));
        poke(level, "hollowScoreboard", new ServerScoreboard(server));
        poke(level, "environmentAttributes",
            net.minecraft.world.attribute.EnvironmentAttributeSystem.builder().build());
        return level;
    }

    /// A level that answers what an entity constructor asks and nothing more.
    private static final class HollowLevel extends net.minecraft.server.level.ServerLevel {
        private RegistryAccess registries;
        private MinecraftServer hollowServer;
        private LevelData hollowLevelData;
        private ServerScoreboard hollowScoreboard;

        // Never called: the class is allocated without running a constructor. It exists only
        // because a subclass has to name one of its superclass's.
        private HollowLevel() {
            super(null, null, null, null, null, null, false, 0L, null, false);
        }

        public int getNextEntityId() {
            return 0;
        }

        @Override
        public boolean isClientSide() {
            return false;
        }

        @Override
        public RegistryAccess registryAccess() {
            return this.registries;
        }

        @Override
        public MinecraftServer getServer() {
            return this.hollowServer;
        }

        @Override
        public FeatureFlagSet enabledFeatures() {
            return FeatureFlags.VANILLA_SET;
        }

        @Override
        public LevelData getLevelData() {
            return this.hollowLevelData;
        }

        @Override
        public ServerScoreboard getScoreboard() {
            return this.hollowScoreboard;
        }

        @Override
        public int getSeaLevel() {
            return 63;
        }

        @Override
        public net.minecraft.world.Difficulty getDifficulty() {
            return net.minecraft.world.Difficulty.NORMAL;
        }

        @Override
        public net.minecraft.core.Holder<net.minecraft.world.level.biome.Biome> getBiome(
            final net.minecraft.core.BlockPos pos
        ) {
            return this.registries
                .lookupOrThrow(net.minecraft.core.registries.Registries.BIOME)
                .getOrThrow(net.minecraft.world.level.biome.Biomes.PLAINS);
        }
    }

    /// Everything a running server would have loaded, loaded the way it loads it.
    ///
    /// A mob that carries an item asks the item registry for that item's components, and those are
    /// only bound once the packs have been read; a mob with a variant asks for a registry that only
    /// exists once they have. So the packs are read here rather than worked around.
    private static RegistryAccess loadEverything() throws Exception {
        PackRepository packs = ServerPacksSource.createVanillaTrustedRepository();
        packs.reload();
        // A fresh repository has nothing selected, and an unselected pack is not opened.
        packs.setSelected(packs.getAvailableIds());

        try (MultiPackResourceManager resources =
                new MultiPackResourceManager(PackType.SERVER_DATA, packs.openAllSelected())) {
            LayeredRegistryAccess<RegistryLayer> layers = RegistryLayer.createRegistryAccess();
            List<Registry.PendingTags<?>> staticTags =
                TagLoader.loadTagsForExistingRegistries(resources, layers.getLayer(RegistryLayer.STATIC));
            // Binding them puts the tags on the registries themselves, so an access built from the
            // layers afterwards knows them too.
            staticTags.forEach(Registry.PendingTags::apply);
            List<HolderLookup.RegistryLookup<?>> context = TagLoader.buildUpdatedLookups(
                layers.getAccessForLoading(RegistryLayer.WORLDGEN), staticTags);
            RegistryAccess.Frozen loaded = RegistryDataLoader.load(
                resources, context, RegistryDataLoader.WORLDGEN_REGISTRIES, ForkJoinPool.commonPool()
            ).join();
            RegistryAccess.Frozen everything =
                layers.replaceFrom(RegistryLayer.WORLDGEN, loaded).compositeAccess();
            BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(everything)
                .forEach(DataComponentInitializers.PendingComponents::apply);
            return everything;
        }
    }

    /// A player, which the registry has no factory for because the server makes its own.
    ///
    /// The one the server makes adds no fields of its own, so what this lays out is what a real
    /// player lays out.
    private static final class HollowPlayer extends net.minecraft.world.entity.player.Player {
        private HollowPlayer(final Level level) {
            super(level, new com.mojang.authlib.GameProfile(java.util.UUID.randomUUID(), "hollow"));
        }

        @Override
        public net.minecraft.world.level.GameType gameMode() {
            return net.minecraft.world.level.GameType.SURVIVAL;
        }

        @Override
        public net.minecraft.world.item.component.ResolvableProfile getProfile() {
            throw new UnsupportedOperationException();
        }
    }

    /// An instance of a class with none of its constructors run and every field left at zero.
    @SuppressWarnings("unchecked")
    private static <T> T hollow(final Class<? extends T> clazz) throws Exception {
        return (T) unsafe().allocateInstance(clazz);
    }

    /// Writes a field that was never assigned, final or not.
    private static void poke(final Object target, final String name, final Object value)
            throws Exception {
        Class<?> clazz = target.getClass();
        while (clazz != null) {
            try {
                Field field = clazz.getDeclaredField(name);
                Unsafe unsafe = unsafe();
                unsafe.putObject(target, unsafe.objectFieldOffset(field), value);
                return;
            } catch (NoSuchFieldException missing) {
                clazz = clazz.getSuperclass();
            }
        }
        throw new NoSuchFieldException(name + " on " + target.getClass());
    }

    private static Unsafe unsafe() throws Exception {
        Field field = Unsafe.class.getDeclaredField("theUnsafe");
        field.setAccessible(true);
        return (Unsafe) field.get(null);
    }
}
