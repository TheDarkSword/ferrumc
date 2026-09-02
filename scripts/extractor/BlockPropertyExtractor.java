// Dumps what each block state is like to break: how hard it is, and whether the right tool is
// needed for it to drop anything.
//
// Neither is in any report. Both live on the block's behaviour, set in code when the block is
// built, and both are per state rather than per block — a lit furnace and an unlit one are two
// states and need not agree.
//
// Keyed by state id, which is what the world stores and what the digging code has to hand.
//
// Since 26.1 the server jar ships with its own names, so this compiles straight against it.

import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;

public final class BlockPropertyExtractor {
    public static void main(final String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        List<String> entries = new ArrayList<>();
        for (BlockState state : Block.BLOCK_STATE_REGISTRY) {
            int id = Block.BLOCK_STATE_REGISTRY.getId(state);
            // Asked of the state with no level and no position: neither is read for either of
            // these, and building a level to ask would change nothing.
            float hardness = state.getDestroySpeed(null, null);
            entries.add(String.format(
                    "  \"%d\": {\"hardness\": %s, \"needs_the_right_tool\": %s, \"light\": %d}",
                    id, hardness, state.requiresCorrectToolForDrops(), state.getLightEmission()));
        }

        try (PrintWriter out = new PrintWriter(
                Files.newBufferedWriter(Path.of(args[0]), StandardCharsets.UTF_8))) {
            out.println("{");
            for (int i = 0; i < entries.size(); i++) {
                out.println(entries.get(i) + (i + 1 < entries.size() ? "," : ""));
            }
            out.println("}");
        }
        System.out.println(entries.size() + " block states");
    }
}
