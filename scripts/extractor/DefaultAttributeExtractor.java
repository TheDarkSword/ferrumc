// Dumps which attributes each kind of entity starts with, and what it starts them at.
//
// A zombie has twenty health, a movement speed of 0.23 and a follow range of 35; none of that is in
// any report, because it is built in code by each entity class rather than written in data. The
// game keeps it in a table keyed by entity type, which is what this reads.
//
// Only things that live have attributes at all: an arrow, a boat and a dropped item have none, and
// asking for theirs is an error rather than an empty answer.
//
// Since 26.1 the server jar ships with its own names, so this compiles straight against it.

import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeInstance;
import net.minecraft.world.entity.ai.attributes.AttributeSupplier;
import net.minecraft.world.entity.ai.attributes.DefaultAttributes;

public final class DefaultAttributeExtractor {
    public static void main(final String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        // The table is private, so it is read straight off the supplier rather than by asking for
        // each of the forty attributes in turn and guessing which answers mean "not set".
        Field field = AttributeSupplier.class.getDeclaredField("instances");
        field.setAccessible(true);

        List<String> entries = new ArrayList<>();
        List<String> failures = new ArrayList<>();
        for (EntityType<?> type : BuiltInRegistries.ENTITY_TYPE) {
            Identifier name = BuiltInRegistries.ENTITY_TYPE.getKey(type);
            if (!DefaultAttributes.hasSupplier(type)) {
                continue;
            }
            try {
                @SuppressWarnings("unchecked")
                AttributeSupplier supplier =
                        DefaultAttributes.getSupplier((EntityType<? extends LivingEntity>) type);
                entries.add(one(name, supplier, field));
            } catch (Throwable failure) {
                failures.add(name + ": " + failure);
            }
        }

        try (PrintWriter out = new PrintWriter(
                Files.newBufferedWriter(Path.of(args[0]), StandardCharsets.UTF_8))) {
            out.println("{");
            for (int i = 0; i < entries.size(); i++) {
                out.println(entries.get(i) + (i + 1 < entries.size() ? "," : ""));
            }
            out.println("}");
        }

        for (String failure : failures) {
            System.err.println("could not read " + failure);
        }
        System.out.println(entries.size() + " kinds with attributes");
    }

    private static String one(final Identifier name, final AttributeSupplier supplier,
            final Field field) throws Exception {
        @SuppressWarnings("unchecked")
        Map<Holder<Attribute>, AttributeInstance> instances =
                (Map<Holder<Attribute>, AttributeInstance>) field.get(supplier);

        // Sorted, so two runs of this produce the same file.
        Map<String, Double> sorted = new TreeMap<>();
        for (Map.Entry<Holder<Attribute>, AttributeInstance> entry : instances.entrySet()) {
            Identifier attribute = BuiltInRegistries.ATTRIBUTE.getKey(entry.getKey().value());
            sorted.put(attribute.getPath(), entry.getValue().getBaseValue());
        }

        StringBuilder out = new StringBuilder("  \"" + name.getPath() + "\": {");
        boolean first = true;
        for (Map.Entry<String, Double> entry : sorted.entrySet()) {
            if (!first) {
                out.append(", ");
            }
            first = false;
            out.append("\"").append(entry.getKey()).append("\": ").append(entry.getValue());
        }
        return out.append("}").toString();
    }
}
