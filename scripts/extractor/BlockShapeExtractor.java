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
import net.minecraft.world.level.lighting.LightEngine;
import net.minecraft.world.phys.shapes.Shapes;
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

    /// Every distinct face occlusion shape, so a state's faces can be indices into it.
    private static final Map<String, Integer> FACE_SHAPES = new LinkedHashMap<>();
    private static final List<VoxelShape> FACE_SHAPE_ORDER = new ArrayList<>();

    private static int faceShape(final VoxelShape shape) {
        final StringBuilder key = new StringBuilder();
        for (final AABB box : shape.toAabbs()) {
            key.append(String.format(
                "[%s %s %s %s %s %s]",
                box.minX, box.minY, box.minZ, box.maxX, box.maxY, box.maxZ
            ));
        }
        final Integer seen = FACE_SHAPES.get(key.toString());
        if (seen != null) {
            return seen;
        }
        FACE_SHAPES.put(key.toString(), FACE_SHAPES.size());
        FACE_SHAPE_ORDER.add(shape);
        return FACE_SHAPES.size() - 1;
    }

    /// Which face shape each of a state's sides has, one index per direction.
    private static String faceShapeIndices(final BlockState state) {
        final StringBuilder out = new StringBuilder("[");
        boolean first = true;
        for (final Direction direction : Direction.values()) {
            if (!first) {
                out.append(',');
            }
            out.append(faceShape(LightEngine.getOcclusionShape(state, direction)));
            first = false;
        }
        return out.append(']').toString();
    }

    /// Which faces of this block stop light on their own.
    ///
    /// Whether light passes between two blocks is really a question about both their faces
    /// together, which is a pair and cannot be tabulated. What is tabulated is each face's own
    /// answer, which settles every case but two partial faces that only cover the opening between
    /// them.
    private static int faceOccludesLight(final BlockState state) {
        int bits = 0;
        int index = 0;
        for (final Direction direction : Direction.values()) {
            final VoxelShape face = LightEngine.getOcclusionShape(state, direction);
            if (Shapes.faceShapeOccludes(face, Shapes.empty())) {
                bits |= 1 << index;
            }
            index++;
        }
        return bits;
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
                "{\"collision\":%d,\"outline\":%d,\"face_sturdy\":%d,\"light_emission\":%d,"
                    + "\"light_dampening\":%d,\"shape_occludes_light\":%b,\"propagates_skylight\":%b,"
                    + "\"face_occludes_light\":%d,\"face_shapes\":%s,"
                    + "\"hardness\":%s,"
                    + "\"air\":%b,\"solid\":%b,\"occludes\":%b,\"randomly_ticking\":%b,"
                    + "\"needs_tool\":%b,\"push_reaction\":\"%s\"}",
                shape(state.getCollisionShape(EmptyBlockGetter.INSTANCE, origin)),
                shape(state.getShape(EmptyBlockGetter.INSTANCE, origin)),
                faceSturdy(state, origin),
                state.getLightEmission(),
                state.getLightDampening(),
                state.useShapeForLightOcclusion(),
                state.propagatesSkylightDown(),
                faceOccludesLight(state),
                faceShapeIndices(state),
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
            // Whether light is stopped between a pair of faces. Which pair a position needs is only
            // known at run time, and there are few enough shapes that every answer fits in a table.
            writer.print("],\"face_occlusion_pairs\":[");
            final List<String> rows = new ArrayList<>();
            for (final VoxelShape from : FACE_SHAPE_ORDER) {
                final StringBuilder row = new StringBuilder("[");
                boolean firstInRow = true;
                for (final VoxelShape to : FACE_SHAPE_ORDER) {
                    if (!firstInRow) {
                        row.append(',');
                    }
                    row.append(Shapes.faceShapeOccludes(from, to) ? 1 : 0);
                    firstInRow = false;
                }
                rows.add(row.append(']').toString());
            }
            writer.print(String.join(",", rows));
            writer.print("],\"states\":[");
            writer.print(String.join(",", states));
            writer.print("]}");
        }
        System.out.printf(
            "%d states, %d shapes, %d boxes -> %s%n", states.size(), SHAPES.size(), BOXES.size(), out
        );
    }
}
