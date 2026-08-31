// Builds every entity the game has, without a world for them to be in.
//
// Almost nothing about an entity is written down: how heavy it is, how much air slows it, how tall
// a step it can take are all methods on the class rather than entries in a report. Reading them
// means having one, and having one means a level for it to be built in — so there is a level here
// that answers exactly what a constructor asks and nothing more.
//
// The entities the level cannot satisfy throw, and whoever asked for one is told so.

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.List;
import java.util.concurrent.ForkJoinPool;

import net.minecraft.core.HolderLookup;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.component.DataComponentInitializers;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.ServerScoreboard;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagLoader;
import net.minecraft.util.debug.ServerDebugSubscribers;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.flag.FeatureFlagSet;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.storage.LevelData;
import net.minecraft.world.level.storage.PrimaryLevelData;
import sun.misc.Unsafe;

public final class GameEntities {
    private final RegistryAccess registries;
    private final Level level;

    public GameEntities() throws Exception {
        this.registries = loadEverything();
        this.level = hollowLevel(this.registries);
    }

    public RegistryAccess registries() {
        return this.registries;
    }

    /// One entity of this type, built the way the game builds one.
    public Entity build(final EntityType<?> type, final Identifier name) throws Exception {
        Entity entity = name.getPath().equals("player")
            ? new HollowPlayer(this.level)
            : type.create(this.level, EntitySpawnReason.COMMAND);
        if (entity == null) {
            throw new IllegalStateException("no factory result");
        }
        return entity;
    }

    /// Calls a method the game keeps to itself.
    public static Object ask(final Object target, final String name) throws Exception {
        for (Class<?> clazz = target.getClass(); clazz != null; clazz = clazz.getSuperclass()) {
            try {
                Method method = clazz.getDeclaredMethod(name);
                method.setAccessible(true);
                return method.invoke(target);
            } catch (NoSuchMethodException missing) {
                // Keep walking up: an override lives on the subclass, the rest on a parent.
            }
        }
        throw new NoSuchMethodException(name + " on " + target.getClass());
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
            net.minecraft.core.registries.BuiltInRegistries.DATA_COMPONENT_INITIALIZERS
                .build(everything)
                .forEach(DataComponentInitializers.PendingComponents::apply);
            return everything;
        }
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
