// Dumps what an entity type is, which no report carries.
//
// The registries report gives a name and a number and nothing else. How big an entity is, how far
// it is tracked, how often it is updated, which category it spawns in and what it drops all live on
// the type itself, and the spawn placement lives in a table beside it. So the game is asked.
//
// Since 26.1 the server jar ships with its own names, so this compiles straight against it.

import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import net.minecraft.SharedConstants;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.entity.EntityDimensions;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.MobCategory;
import net.minecraft.world.entity.SpawnPlacementTypes;
import net.minecraft.world.entity.SpawnPlacements;
import net.minecraft.world.entity.ai.attributes.AttributeSupplier;
import net.minecraft.world.entity.ai.attributes.Attributes;
import net.minecraft.world.entity.ai.attributes.DefaultAttributes;

public final class EntityTypeExtractor {
    public static void main(final String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        try (PrintWriter out = new PrintWriter(
                Files.newBufferedWriter(Path.of(args[0]), StandardCharsets.UTF_8))) {
            out.println("{");
            out.println("  \"version\": \"" + SharedConstants.getCurrentVersion().name() + "\",");
            out.println("  \"categories\": {");
            MobCategory[] categories = MobCategory.values();
            for (int i = 0; i < categories.length; i++) {
                MobCategory category = categories[i];
                out.println("    \"" + category.getName() + "\": {"
                    + "\"max_per_chunk\": " + category.getMaxInstancesPerChunk() + ", "
                    + "\"friendly\": " + category.isFriendly() + ", "
                    + "\"persistent\": " + category.isPersistent() + ", "
                    + "\"despawn_distance\": " + category.getDespawnDistance() + ", "
                    + "\"no_despawn_distance\": " + category.getNoDespawnDistance()
                    + "}" + (i + 1 < categories.length ? "," : ""));
            }
            out.println("  },");
            out.println("  \"types\": [");

            List<EntityType<?>> types = BuiltInRegistries.ENTITY_TYPE.stream().toList();
            for (int i = 0; i < types.size(); i++) {
                EntityType<?> type = types.get(i);
                Identifier name = BuiltInRegistries.ENTITY_TYPE.getKey(type);
                EntityDimensions size = type.getDimensions();
                out.println("    {");
                out.println("      \"id\": " + BuiltInRegistries.ENTITY_TYPE.getId(type) + ",");
                out.println("      \"name\": \"" + name + "\",");
                out.println("      \"category\": \"" + type.getCategory().getName() + "\",");
                out.println("      \"width\": " + size.width() + ",");
                out.println("      \"height\": " + size.height() + ",");
                out.println("      \"eye_height\": " + size.eyeHeight() + ",");
                out.println("      \"fixed_size\": " + size.fixed() + ",");
                out.println("      \"tracking_range\": " + type.clientTrackingRange() + ",");
                out.println("      \"update_interval\": " + type.updateInterval() + ",");
                out.println("      \"serialize\": " + type.canSerialize() + ",");
                out.println("      \"summon\": " + type.canSummon() + ",");
                out.println("      \"fire_immune\": " + type.fireImmune() + ",");
                out.println("      \"spawn_far_from_player\": " + type.canSpawnFarFromPlayer() + ",");
                out.println("      \"allowed_in_peaceful\": " + type.isAllowedInPeaceful() + ",");
                out.println("      \"placement\": \"" + placement(type) + "\",");
                out.println("      \"heightmap\": \"" + SpawnPlacements.getHeightmapType(type) + "\",");
                out.println("      \"max_health\": " + maxHealth(type));
                out.println("    }" + (i + 1 < types.size() ? "," : ""));
            }
            out.println("  ]");
            out.println("}");
        }
    }

    /// Where a mob of this type may stand: on ground, in water, in lava, or anywhere.
    ///
    /// They are lambdas with no name of their own, so which one it is has to be asked by identity
    /// rather than read off a `toString`.
    private static String placement(final EntityType<?> type) {
        Object placement = SpawnPlacements.getPlacementType(type);
        if (placement == SpawnPlacementTypes.ON_GROUND) {
            return "on_ground";
        }
        if (placement == SpawnPlacementTypes.IN_WATER) {
            return "in_water";
        }
        if (placement == SpawnPlacementTypes.IN_LAVA) {
            return "in_lava";
        }
        if (placement == SpawnPlacementTypes.NO_RESTRICTIONS) {
            return "no_restrictions";
        }
        throw new IllegalStateException("unknown spawn placement for " + type);
    }

    /// What a living entity of this type starts with, or nothing where it is not a living one.
    @SuppressWarnings("unchecked")
    private static String maxHealth(final EntityType<?> type) {
        if (!DefaultAttributes.hasSupplier(type)) {
            return "null";
        }
        AttributeSupplier attributes =
            DefaultAttributes.getSupplier((EntityType<? extends LivingEntity>) type);
        if (!attributes.hasAttribute(Attributes.MAX_HEALTH)) {
            return "null";
        }
        return Double.toString(attributes.getValue(Attributes.MAX_HEALTH));
    }
}
