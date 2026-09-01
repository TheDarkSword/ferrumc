// Dumps every attribute: the number it travels as, what it starts at, and how far it may be moved.
//
// The registries report gives a name and a number, and nothing else. What an attribute starts at
// and the range it is held to live on the attribute object, which is why they are asked of the game
// rather than read out of a file.
//
// Since 26.1 the server jar ships with its own names, so this compiles straight against it.

import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

import net.minecraft.SharedConstants;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.RangedAttribute;

public final class AttributeExtractor {
    public static void main(final String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        List<String> entries = new ArrayList<>();
        for (Attribute attribute : BuiltInRegistries.ATTRIBUTE) {
            Identifier name = BuiltInRegistries.ATTRIBUTE.getKey(attribute);
            int id = BuiltInRegistries.ATTRIBUTE.getId(attribute);
            double lowest = Double.NEGATIVE_INFINITY;
            double highest = Double.POSITIVE_INFINITY;
            if (attribute instanceof RangedAttribute ranged) {
                lowest = ranged.getMinValue();
                highest = ranged.getMaxValue();
            }
            entries.add(String.format(
                    "  \"%s\": {\"id\": %d, \"default_value\": %s, \"lowest\": %s,"
                            + " \"highest\": %s, \"syncable\": %s}",
                    name.getPath(), id, number(attribute.getDefaultValue()), number(lowest),
                    number(highest), attribute.isClientSyncable()));
        }

        try (PrintWriter out = new PrintWriter(
                Files.newBufferedWriter(Path.of(args[0]), StandardCharsets.UTF_8))) {
            out.println("{");
            for (int i = 0; i < entries.size(); i++) {
                out.println(entries.get(i) + (i + 1 < entries.size() ? "," : ""));
            }
            out.println("}");
        }
        System.out.println(entries.size() + " attributes");
    }

    // JSON has no infinity, so an unbounded end is written as the largest double there is.
    private static String number(final double value) {
        if (Double.isInfinite(value)) {
            return value > 0 ? String.valueOf(Double.MAX_VALUE) : String.valueOf(-Double.MAX_VALUE);
        }
        return String.valueOf(value);
    }
}
