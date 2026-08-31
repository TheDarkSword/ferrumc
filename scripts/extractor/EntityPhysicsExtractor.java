// Dumps how an entity moves, which no report carries and no file holds.
//
// How heavy an entity is, how much air slows it, how tall a step it can take and whether it is
// pushed around by water are all methods on the entity class. Several of them read an attribute
// that a datapack can change, so they are asked of a built entity rather than looked up on the
// type. What comes back is what the game would use on the first tick of a freshly spawned one.
//
// Living entities and everything else move in a different order — a mob has its drag applied
// before it moves and an item after — and which of the two an entity is has to come from the game
// as well, since nothing about the type says so.
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
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;

public final class EntityPhysicsExtractor {
    public static void main(final String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        GameEntities game = new GameEntities();

        List<String> entries = new ArrayList<>();
        List<String> failures = new ArrayList<>();
        List<EntityType<?>> types = BuiltInRegistries.ENTITY_TYPE.stream().toList();
        for (EntityType<?> type : types) {
            Identifier name = BuiltInRegistries.ENTITY_TYPE.getKey(type);
            try {
                entries.add(physics(type, name, game));
            } catch (Throwable failure) {
                failures.add(name + ": " + failure);
            }
        }

        try (PrintWriter out = new PrintWriter(
                Files.newBufferedWriter(Path.of(args[0]), StandardCharsets.UTF_8))) {
            out.println("{");
            out.println("  \"version\": \"" + SharedConstants.getCurrentVersion().name() + "\",");
            out.println("  \"types\": {");
            for (int i = 0; i < entries.size(); i++) {
                out.println(entries.get(i) + (i + 1 < entries.size() ? "," : ""));
            }
            out.println("  }");
            out.println("}");
        }

        for (String failure : failures) {
            System.err.println("could not build " + failure);
        }
        System.err.println(entries.size() + " of " + types.size() + " types read");
    }

    private static String physics(
        final EntityType<?> type, final Identifier name, final GameEntities game
    ) throws Exception {
        Entity entity = game.build(type, name);
        // Gravity is the effective one rather than the plain one: a falling living entity with
        // slow falling is lighter, and asking for the effective one is asking the same question
        // the tick asks.
        double gravity = (Double) GameEntities.ask(entity, "getGravity");
        float airDrag = (Float) GameEntities.ask(entity, "getAirDrag");
        float stepHeight = entity.maxUpStep();
        boolean living = entity instanceof LivingEntity;
        boolean omnidirectional = (Boolean) GameEntities.ask(entity, "omnidirectionalAirMover");
        boolean pushedByFluid = entity.isPushedByFluid();

        return "    \"" + name + "\": {"
            + "\"gravity\": " + gravity + ", "
            + "\"air_drag\": " + airDrag + ", "
            + "\"step_height\": " + stepHeight + ", "
            + "\"living\": " + living + ", "
            + "\"omnidirectional\": " + omnidirectional + ", "
            + "\"pushed_by_fluid\": " + pushedByFluid
            + "}";
    }
}
