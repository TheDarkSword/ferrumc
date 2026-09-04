// Dumps what each item leaves behind when it is crafted with.
//
// A bucket of milk in a cake recipe leaves the bucket. Which items do that is a field on the item,
// set in code when it is built, and it is *not* the `use_remainder` component — that one is what
// drinking something leaves, which is a different list that happens to overlap.
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
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStackTemplate;

public final class CraftingRemainderExtractor {
    public static void main(final String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        List<String> entries = new ArrayList<>();
        for (Item item : BuiltInRegistries.ITEM) {
            ItemStackTemplate remainder = item.getCraftingRemainder();
            if (remainder == null) {
                continue;
            }
            Identifier name = BuiltInRegistries.ITEM.getKey(item);
            // Asked of the template rather than of a stack made from it: making one needs the
            // component registry bound, which happens later than this runs.
            Identifier left = BuiltInRegistries.ITEM.getKey(remainder.item().value());
            entries.add(String.format("  \"%s\": \"%s\"", name.getPath(), left.getPath()));
        }

        try (PrintWriter out = new PrintWriter(
                Files.newBufferedWriter(Path.of(args[0]), StandardCharsets.UTF_8))) {
            out.println("{");
            for (int i = 0; i < entries.size(); i++) {
                out.println(entries.get(i) + (i + 1 < entries.size() ? "," : ""));
            }
            out.println("}");
        }
        System.out.println(entries.size() + " items that leave something behind");
    }
}
