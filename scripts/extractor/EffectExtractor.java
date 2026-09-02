// Dumps every status effect: the number it travels as, what it is called, what colour a client
// draws it, whether it helps or hurts, whether it lands all at once, and which attributes it moves.
//
// The attributes are the part no report carries. An effect's modifier is a base amount that is
// multiplied by the amplifier, so speed I and speed II are the same modifier at two strengths, and
// what that amount is lives on the effect object rather than in any file.
//
// Since 26.1 the server jar ships with its own names, so this compiles straight against it.

import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

import net.minecraft.SharedConstants;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.effect.InstantaneousMobEffect;
import net.minecraft.world.effect.MobEffect;

public final class EffectExtractor {
    public static void main(final String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        List<String> entries = new ArrayList<>();
        for (MobEffect effect : BuiltInRegistries.MOB_EFFECT) {
            Identifier name = BuiltInRegistries.MOB_EFFECT.getKey(effect);
            int id = BuiltInRegistries.MOB_EFFECT.getId(effect);
            entries.add(one(name, id, effect));
        }

        try (PrintWriter out = new PrintWriter(
                Files.newBufferedWriter(Path.of(args[0]), StandardCharsets.UTF_8))) {
            out.println("{");
            for (int i = 0; i < entries.size(); i++) {
                out.println(entries.get(i) + (i + 1 < entries.size() ? "," : ""));
            }
            out.println("}");
        }
        System.out.println(entries.size() + " effects");
    }

    private static String one(final Identifier name, final int id, final MobEffect effect) {
        // Sorted, so two runs of this produce the same file. The effect is asked for the modifiers
        // a level-one instance would carry, since that is the amount one level is worth and what
        // every level after it is a multiple of.
        Map<String, String> sorted = new TreeMap<>();
        effect.createModifiers(0, (attribute, modifier) -> {
            Identifier named = BuiltInRegistries.ATTRIBUTE.getKey(attribute.value());
            sorted.put(named.getPath(), String.format(
                    "{\"amount\": %s, \"operation\": \"%s\", \"id\": \"%s\"}",
                    modifier.amount(), modifier.operation().getSerializedName(), modifier.id()));
        });

        StringBuilder attributes = new StringBuilder("{");
        boolean first = true;
        for (Map.Entry<String, String> entry : sorted.entrySet()) {
            if (!first) {
                attributes.append(", ");
            }
            first = false;
            attributes.append("\"").append(entry.getKey()).append("\": ").append(entry.getValue());
        }
        attributes.append("}");

        return String.format(
                "  \"%s\": {\"id\": %d, \"category\": \"%s\", \"color\": %d,"
                        + " \"instant\": %s, \"attributes\": %s}",
                name.getPath(), id, effect.getCategory().name().toLowerCase(java.util.Locale.ROOT),
                effect.getColor(), effect instanceof InstantaneousMobEffect, attributes);
    }
}
