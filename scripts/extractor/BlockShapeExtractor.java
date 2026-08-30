// Dumps what the block report leaves out: the shapes a block state occupies, and the numbers that
// live in code rather than in data.
//
// Collision shapes are built by the blocks themselves, sometimes from lambdas over their property
// values, so there is no way to read them out of a file. The game has to be asked. Since 26.1 the
// server jar ships with its own names, so this compiles straight against it.
//
// Shapes that depend on a block's neighbours are asked against an empty world, which is what a
// block reports when it stands alone.

import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.core.Direction;
import net.minecraft.world.level.block.SupportType;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.shapes.VoxelShape;

public final class BlockShapeExtractor {
    /// Distinct boxes, so the thousands of states that share one do not each carry a copy.
    private static final Map<String, Integer> BOXES = new LinkedHashMap<>();
    /// Distinct shapes, each a list of box indices.
    private static final Map<String, Integer> SHAPES = new LinkedHashMap<>();
    private static final List<String> BOX_ORDER = new ArrayList<>();
    private static final List<String> SHAPE_ORDER = new ArrayList<>();

    private static int box(final AABB aabb) {
        final String key = String.format(
            "[%s,%s,%s,%s,%s,%s]",
            trim(aabb.minX), trim(aabb.minY), trim(aabb.minZ),
            trim(aabb.maxX), trim(aabb.maxY), trim(aabb.maxZ)
        );
        final Integer seen = BOXES.get(key);
        if (seen != null) {
            return seen;
        }
        BOXES.put(key, BOXES.size());
        BOX_ORDER.add(key);
        return BOXES.size() - 1;
    }

    private static int shape(final VoxelShape voxel) {
        final StringBuilder builder = new StringBuilder("[");
        boolean first = true;
        for (final AABB aabb : voxel.toAabbs()) {
            if (!first) {
                builder.append(',');
            }
            builder.append(box(aabb));
            first = false;
        }
        builder.append(']');
        final String key = builder.toString();
        final Integer seen = SHAPES.get(key);
        if (seen != null) {
            return seen;
        }
        SHAPES.put(key, SHAPES.size());
        SHAPE_ORDER.add(key);
        return SHAPES.size() - 1;
    }

    /// Which faces hold something up, and how much of one they hold.
    ///
    /// This is what a torch asks before staying on a block and what a door asks before standing on
    /// one. It is an answer about the block's support shape rather than the shape itself, which is
    /// all any caller wants and saves carrying a fourth shape per state.
    ///
    /// One bit per direction and support type, in the game's own order.
    private static int faceSturdy(final BlockState state, final BlockPos pos) {
        int bits = 0;
        int index = 0;
        for (final Direction direction : Direction.values()) {
            for (final SupportType support : SupportType.values()) {
                if (state.isFaceSturdy(EmptyBlockGetter.INSTANCE, pos, direction, support)) {
                    bits |= 1 << index;
                }
                index++;
            }
        }
        return bits;
    }

    /// Whole numbers as integers, so the output does not fill with trailing zeroes.
    private static String trim(final double value) {
        return value == Math.rint(value) ? Long.toString((long) value) : Double.toString(value);
    }

    public static void main(final String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        final BlockPos origin = BlockPos.ZERO;
        final List<String> states = new ArrayList<>();
        int highest = -1;
        for (final BlockState state : Block.BLOCK_STATE_REGISTRY) {
            highest = Math.max(highest, Block.BLOCK_STATE_REGISTRY.getId(state));
        }
        for (int id = 0; id <= highest; id++) {
            final BlockState state = Block.BLOCK_STATE_REGISTRY.byId(id);
            if (state == null) {
                states.add("null");
                continue;
            }
            states.add(String.format(
                "{\"collision\":%d,\"outline\":%d,\"face_sturdy\":%d,\"light_emission\":%d,\"hardness\":%s,"
                    + "\"air\":%b,\"solid\":%b,\"occludes\":%b,\"randomly_ticking\":%b,"
                    + "\"needs_tool\":%b,\"push_reaction\":\"%s\"}",
                shape(state.getCollisionShape(EmptyBlockGetter.INSTANCE, origin)),
                shape(state.getShape(EmptyBlockGetter.INSTANCE, origin)),
                faceSturdy(state, origin),
                state.getLightEmission(),
                trim(state.getDestroySpeed(EmptyBlockGetter.INSTANCE, origin)),
                state.isAir(),
                state.isSolid(),
                state.canOcclude(),
                state.isRandomlyTicking(),
                state.requiresCorrectToolForDrops(),
                state.getPistonPushReaction()
            ));
        }

        final Path out = Path.of(args[0]);
        try (PrintWriter writer = new PrintWriter(Files.newBufferedWriter(out, StandardCharsets.UTF_8))) {
            writer.print("{\"version\":\"");
            writer.print(SharedConstants.getCurrentVersion().name());
            writer.print("\",\"boxes\":[");
            writer.print(String.join(",", BOX_ORDER));
            writer.print("],\"shapes\":[");
            writer.print(String.join(",", SHAPE_ORDER));
            writer.print("],\"states\":[");
            writer.print(String.join(",", states));
            writer.print("]}");
        }
        System.out.printf(
            "%d states, %d shapes, %d boxes -> %s%n", states.size(), SHAPES.size(), BOXES.size(), out
        );
    }
}
