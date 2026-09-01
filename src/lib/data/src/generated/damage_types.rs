#[doc = r" When the difficulty moves how hard a kind of damage hits."]
#[doc = r""]
#[doc = r" Only a mob's blow is softened on easy and sharpened on hard; a player's blow and the"]
#[doc = r" world's own hazards land the same whatever the difficulty."]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scaling {
    #[doc = r" Never, whatever the difficulty."]
    Never,
    #[doc = r" Only when something living that is not a player is behind it."]
    WhenCausedByLivingNonPlayer,
    #[doc = r" Always."]
    Always,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageType {
    Arrow,
    BadRespawnPoint,
    Cactus,
    Campfire,
    Cramming,
    DragonBreath,
    Drown,
    DryOut,
    EnderPearl,
    Explosion,
    Fall,
    FallingAnvil,
    FallingBlock,
    FallingStalactite,
    Fireball,
    Fireworks,
    FlyIntoWall,
    Freeze,
    Generic,
    GenericKill,
    HotFloor,
    InFire,
    InWall,
    IndirectMagic,
    Lava,
    LightningBolt,
    MaceSmash,
    Magic,
    MobAttack,
    MobAttackNoAggro,
    MobProjectile,
    OnFire,
    OutOfWorld,
    OutsideBorder,
    PlayerAttack,
    PlayerExplosion,
    SonicBoom,
    Spear,
    Spit,
    Stalagmite,
    Starve,
    Sting,
    SulfurCubeHot,
    SweetBerryBush,
    Thorns,
    Thrown,
    Trident,
    UnattributedFireball,
    WindCharge,
    Wither,
    WitherSkull,
}
impl DamageType {
    #[doc = r" Try to parse a `DamageType` from a resource location string."]
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        match name {
            "arrow" => Some(Self::Arrow),
            "bad_respawn_point" => Some(Self::BadRespawnPoint),
            "cactus" => Some(Self::Cactus),
            "campfire" => Some(Self::Campfire),
            "cramming" => Some(Self::Cramming),
            "dragon_breath" => Some(Self::DragonBreath),
            "drown" => Some(Self::Drown),
            "dry_out" => Some(Self::DryOut),
            "ender_pearl" => Some(Self::EnderPearl),
            "explosion" => Some(Self::Explosion),
            "fall" => Some(Self::Fall),
            "falling_anvil" => Some(Self::FallingAnvil),
            "falling_block" => Some(Self::FallingBlock),
            "falling_stalactite" => Some(Self::FallingStalactite),
            "fireball" => Some(Self::Fireball),
            "fireworks" => Some(Self::Fireworks),
            "fly_into_wall" => Some(Self::FlyIntoWall),
            "freeze" => Some(Self::Freeze),
            "generic" => Some(Self::Generic),
            "generic_kill" => Some(Self::GenericKill),
            "hot_floor" => Some(Self::HotFloor),
            "in_fire" => Some(Self::InFire),
            "in_wall" => Some(Self::InWall),
            "indirect_magic" => Some(Self::IndirectMagic),
            "lava" => Some(Self::Lava),
            "lightning_bolt" => Some(Self::LightningBolt),
            "mace_smash" => Some(Self::MaceSmash),
            "magic" => Some(Self::Magic),
            "mob_attack" => Some(Self::MobAttack),
            "mob_attack_no_aggro" => Some(Self::MobAttackNoAggro),
            "mob_projectile" => Some(Self::MobProjectile),
            "on_fire" => Some(Self::OnFire),
            "out_of_world" => Some(Self::OutOfWorld),
            "outside_border" => Some(Self::OutsideBorder),
            "player_attack" => Some(Self::PlayerAttack),
            "player_explosion" => Some(Self::PlayerExplosion),
            "sonic_boom" => Some(Self::SonicBoom),
            "spear" => Some(Self::Spear),
            "spit" => Some(Self::Spit),
            "stalagmite" => Some(Self::Stalagmite),
            "starve" => Some(Self::Starve),
            "sting" => Some(Self::Sting),
            "sulfur_cube_hot" => Some(Self::SulfurCubeHot),
            "sweet_berry_bush" => Some(Self::SweetBerryBush),
            "thorns" => Some(Self::Thorns),
            "thrown" => Some(Self::Thrown),
            "trident" => Some(Self::Trident),
            "unattributed_fireball" => Some(Self::UnattributedFireball),
            "wind_charge" => Some(Self::WindCharge),
            "wither" => Some(Self::Wither),
            "wither_skull" => Some(Self::WitherSkull),
            _ => None,
        }
    }
    pub const fn to_name(&self) -> &'static str {
        match self {
            Self::Arrow => "arrow",
            Self::BadRespawnPoint => "bad_respawn_point",
            Self::Cactus => "cactus",
            Self::Campfire => "campfire",
            Self::Cramming => "cramming",
            Self::DragonBreath => "dragon_breath",
            Self::Drown => "drown",
            Self::DryOut => "dry_out",
            Self::EnderPearl => "ender_pearl",
            Self::Explosion => "explosion",
            Self::Fall => "fall",
            Self::FallingAnvil => "falling_anvil",
            Self::FallingBlock => "falling_block",
            Self::FallingStalactite => "falling_stalactite",
            Self::Fireball => "fireball",
            Self::Fireworks => "fireworks",
            Self::FlyIntoWall => "fly_into_wall",
            Self::Freeze => "freeze",
            Self::Generic => "generic",
            Self::GenericKill => "generic_kill",
            Self::HotFloor => "hot_floor",
            Self::InFire => "in_fire",
            Self::InWall => "in_wall",
            Self::IndirectMagic => "indirect_magic",
            Self::Lava => "lava",
            Self::LightningBolt => "lightning_bolt",
            Self::MaceSmash => "mace_smash",
            Self::Magic => "magic",
            Self::MobAttack => "mob_attack",
            Self::MobAttackNoAggro => "mob_attack_no_aggro",
            Self::MobProjectile => "mob_projectile",
            Self::OnFire => "on_fire",
            Self::OutOfWorld => "out_of_world",
            Self::OutsideBorder => "outside_border",
            Self::PlayerAttack => "player_attack",
            Self::PlayerExplosion => "player_explosion",
            Self::SonicBoom => "sonic_boom",
            Self::Spear => "spear",
            Self::Spit => "spit",
            Self::Stalagmite => "stalagmite",
            Self::Starve => "starve",
            Self::Sting => "sting",
            Self::SulfurCubeHot => "sulfur_cube_hot",
            Self::SweetBerryBush => "sweet_berry_bush",
            Self::Thorns => "thorns",
            Self::Thrown => "thrown",
            Self::Trident => "trident",
            Self::UnattributedFireball => "unattributed_fireball",
            Self::WindCharge => "wind_charge",
            Self::Wither => "wither",
            Self::WitherSkull => "wither_skull",
        }
    }
    #[doc = r" The name a death message is written from, which is not the name of the kind: falling is"]
    #[doc = r" `fall`, burning is `onFire`, and being hit by a player is `player`."]
    #[must_use]
    pub const fn message_id(self) -> &'static str {
        match self {
            Self::Arrow => "arrow",
            Self::BadRespawnPoint => "badRespawnPoint",
            Self::Cactus => "cactus",
            Self::Campfire => "inFire",
            Self::Cramming => "cramming",
            Self::DragonBreath => "dragonBreath",
            Self::Drown => "drown",
            Self::DryOut => "dryout",
            Self::EnderPearl => "fall",
            Self::Explosion => "explosion",
            Self::Fall => "fall",
            Self::FallingAnvil => "anvil",
            Self::FallingBlock => "fallingBlock",
            Self::FallingStalactite => "fallingStalactite",
            Self::Fireball => "fireball",
            Self::Fireworks => "fireworks",
            Self::FlyIntoWall => "flyIntoWall",
            Self::Freeze => "freeze",
            Self::Generic => "generic",
            Self::GenericKill => "genericKill",
            Self::HotFloor => "hotFloor",
            Self::InFire => "inFire",
            Self::InWall => "inWall",
            Self::IndirectMagic => "indirectMagic",
            Self::Lava => "lava",
            Self::LightningBolt => "lightningBolt",
            Self::MaceSmash => "mace_smash",
            Self::Magic => "magic",
            Self::MobAttack => "mob",
            Self::MobAttackNoAggro => "mob",
            Self::MobProjectile => "mob",
            Self::OnFire => "onFire",
            Self::OutOfWorld => "outOfWorld",
            Self::OutsideBorder => "outsideBorder",
            Self::PlayerAttack => "player",
            Self::PlayerExplosion => "explosion.player",
            Self::SonicBoom => "sonic_boom",
            Self::Spear => "spear",
            Self::Spit => "mob",
            Self::Stalagmite => "stalagmite",
            Self::Starve => "starve",
            Self::Sting => "sting",
            Self::SulfurCubeHot => "sulfurCubeHot",
            Self::SweetBerryBush => "sweetBerryBush",
            Self::Thorns => "thorns",
            Self::Thrown => "thrown",
            Self::Trident => "trident",
            Self::UnattributedFireball => "onFire",
            Self::WindCharge => "mob",
            Self::Wither => "wither",
            Self::WitherSkull => "witherSkull",
        }
    }
    #[doc = r" Whether the difficulty moves this kind of damage, and when."]
    #[must_use]
    pub const fn scaling(self) -> Scaling {
        match self {
            Self::Arrow => Scaling::WhenCausedByLivingNonPlayer,
            Self::BadRespawnPoint => Scaling::Always,
            Self::Cactus => Scaling::WhenCausedByLivingNonPlayer,
            Self::Campfire => Scaling::WhenCausedByLivingNonPlayer,
            Self::Cramming => Scaling::WhenCausedByLivingNonPlayer,
            Self::DragonBreath => Scaling::WhenCausedByLivingNonPlayer,
            Self::Drown => Scaling::WhenCausedByLivingNonPlayer,
            Self::DryOut => Scaling::WhenCausedByLivingNonPlayer,
            Self::EnderPearl => Scaling::WhenCausedByLivingNonPlayer,
            Self::Explosion => Scaling::Always,
            Self::Fall => Scaling::WhenCausedByLivingNonPlayer,
            Self::FallingAnvil => Scaling::WhenCausedByLivingNonPlayer,
            Self::FallingBlock => Scaling::WhenCausedByLivingNonPlayer,
            Self::FallingStalactite => Scaling::WhenCausedByLivingNonPlayer,
            Self::Fireball => Scaling::WhenCausedByLivingNonPlayer,
            Self::Fireworks => Scaling::WhenCausedByLivingNonPlayer,
            Self::FlyIntoWall => Scaling::WhenCausedByLivingNonPlayer,
            Self::Freeze => Scaling::WhenCausedByLivingNonPlayer,
            Self::Generic => Scaling::WhenCausedByLivingNonPlayer,
            Self::GenericKill => Scaling::WhenCausedByLivingNonPlayer,
            Self::HotFloor => Scaling::WhenCausedByLivingNonPlayer,
            Self::InFire => Scaling::WhenCausedByLivingNonPlayer,
            Self::InWall => Scaling::WhenCausedByLivingNonPlayer,
            Self::IndirectMagic => Scaling::WhenCausedByLivingNonPlayer,
            Self::Lava => Scaling::WhenCausedByLivingNonPlayer,
            Self::LightningBolt => Scaling::WhenCausedByLivingNonPlayer,
            Self::MaceSmash => Scaling::WhenCausedByLivingNonPlayer,
            Self::Magic => Scaling::WhenCausedByLivingNonPlayer,
            Self::MobAttack => Scaling::WhenCausedByLivingNonPlayer,
            Self::MobAttackNoAggro => Scaling::WhenCausedByLivingNonPlayer,
            Self::MobProjectile => Scaling::WhenCausedByLivingNonPlayer,
            Self::OnFire => Scaling::WhenCausedByLivingNonPlayer,
            Self::OutOfWorld => Scaling::WhenCausedByLivingNonPlayer,
            Self::OutsideBorder => Scaling::WhenCausedByLivingNonPlayer,
            Self::PlayerAttack => Scaling::WhenCausedByLivingNonPlayer,
            Self::PlayerExplosion => Scaling::Always,
            Self::SonicBoom => Scaling::Always,
            Self::Spear => Scaling::WhenCausedByLivingNonPlayer,
            Self::Spit => Scaling::WhenCausedByLivingNonPlayer,
            Self::Stalagmite => Scaling::WhenCausedByLivingNonPlayer,
            Self::Starve => Scaling::WhenCausedByLivingNonPlayer,
            Self::Sting => Scaling::WhenCausedByLivingNonPlayer,
            Self::SulfurCubeHot => Scaling::WhenCausedByLivingNonPlayer,
            Self::SweetBerryBush => Scaling::WhenCausedByLivingNonPlayer,
            Self::Thorns => Scaling::WhenCausedByLivingNonPlayer,
            Self::Thrown => Scaling::WhenCausedByLivingNonPlayer,
            Self::Trident => Scaling::WhenCausedByLivingNonPlayer,
            Self::UnattributedFireball => Scaling::WhenCausedByLivingNonPlayer,
            Self::WindCharge => Scaling::WhenCausedByLivingNonPlayer,
            Self::Wither => Scaling::WhenCausedByLivingNonPlayer,
            Self::WitherSkull => Scaling::WhenCausedByLivingNonPlayer,
        }
    }
    #[doc = r" What taking this costs a player in hunger."]
    #[must_use]
    pub const fn exhaustion(self) -> f32 {
        match self {
            Self::Arrow => 0.1f32,
            Self::BadRespawnPoint => 0.1f32,
            Self::Cactus => 0.1f32,
            Self::Campfire => 0.1f32,
            Self::Cramming => 0f32,
            Self::DragonBreath => 0f32,
            Self::Drown => 0f32,
            Self::DryOut => 0.1f32,
            Self::EnderPearl => 0f32,
            Self::Explosion => 0.1f32,
            Self::Fall => 0f32,
            Self::FallingAnvil => 0.1f32,
            Self::FallingBlock => 0.1f32,
            Self::FallingStalactite => 0.1f32,
            Self::Fireball => 0.1f32,
            Self::Fireworks => 0.1f32,
            Self::FlyIntoWall => 0f32,
            Self::Freeze => 0f32,
            Self::Generic => 0f32,
            Self::GenericKill => 0f32,
            Self::HotFloor => 0.1f32,
            Self::InFire => 0.1f32,
            Self::InWall => 0f32,
            Self::IndirectMagic => 0f32,
            Self::Lava => 0.1f32,
            Self::LightningBolt => 0.1f32,
            Self::MaceSmash => 0.1f32,
            Self::Magic => 0f32,
            Self::MobAttack => 0.1f32,
            Self::MobAttackNoAggro => 0.1f32,
            Self::MobProjectile => 0.1f32,
            Self::OnFire => 0f32,
            Self::OutOfWorld => 0f32,
            Self::OutsideBorder => 0f32,
            Self::PlayerAttack => 0.1f32,
            Self::PlayerExplosion => 0.1f32,
            Self::SonicBoom => 0f32,
            Self::Spear => 0.1f32,
            Self::Spit => 0.1f32,
            Self::Stalagmite => 0f32,
            Self::Starve => 0f32,
            Self::Sting => 0.1f32,
            Self::SulfurCubeHot => 0.1f32,
            Self::SweetBerryBush => 0.1f32,
            Self::Thorns => 0.1f32,
            Self::Thrown => 0.1f32,
            Self::Trident => 0.1f32,
            Self::UnattributedFireball => 0.1f32,
            Self::WindCharge => 0.1f32,
            Self::Wither => 0f32,
            Self::WitherSkull => 0.1f32,
        }
    }
    #[doc = " Whether this kind is in the packs' `bypasses_armor` group."]
    #[must_use]
    pub const fn goes_through_armour(self) -> bool {
        matches!(
            self,
            Self::Cramming
                | Self::DragonBreath
                | Self::Drown
                | Self::EnderPearl
                | Self::Fall
                | Self::FlyIntoWall
                | Self::Freeze
                | Self::Generic
                | Self::GenericKill
                | Self::InWall
                | Self::IndirectMagic
                | Self::Magic
                | Self::OnFire
                | Self::OutOfWorld
                | Self::OutsideBorder
                | Self::SonicBoom
                | Self::Stalagmite
                | Self::Starve
                | Self::Wither
        )
    }
    #[doc = " Whether this kind is in the packs' `bypasses_effects` group."]
    #[must_use]
    pub const fn goes_through_effects(self) -> bool {
        matches!(self, Self::Starve)
    }
    #[doc = " Whether this kind is in the packs' `bypasses_resistance` group."]
    #[must_use]
    pub const fn goes_through_resistance(self) -> bool {
        matches!(self, Self::GenericKill | Self::OutOfWorld)
    }
    #[doc = " Whether this kind is in the packs' `bypasses_invulnerability` group."]
    #[must_use]
    pub const fn goes_through_invulnerability(self) -> bool {
        matches!(self, Self::GenericKill | Self::OutOfWorld)
    }
    #[doc = " Whether this kind is in the packs' `bypasses_cooldown` group."]
    #[must_use]
    pub const fn goes_through_the_cooldown(self) -> bool {
        false
    }
    #[doc = " Whether this kind is in the packs' `is_fire` group."]
    #[must_use]
    pub const fn is_fire(self) -> bool {
        matches!(
            self,
            Self::Campfire
                | Self::Fireball
                | Self::HotFloor
                | Self::InFire
                | Self::Lava
                | Self::OnFire
                | Self::SulfurCubeHot
                | Self::UnattributedFireball
        )
    }
    #[doc = " Whether this kind is in the packs' `is_fall` group."]
    #[must_use]
    pub const fn is_fall(self) -> bool {
        matches!(self, Self::EnderPearl | Self::Fall | Self::Stalagmite)
    }
    #[doc = " Whether this kind is in the packs' `is_drowning` group."]
    #[must_use]
    pub const fn is_drowning(self) -> bool {
        matches!(self, Self::Drown)
    }
    #[doc = " Whether this kind is in the packs' `is_explosion` group."]
    #[must_use]
    pub const fn is_explosion(self) -> bool {
        matches!(
            self,
            Self::BadRespawnPoint | Self::Explosion | Self::Fireworks | Self::PlayerExplosion
        )
    }
    #[doc = " Whether this kind is in the packs' `no_knockback` group."]
    #[must_use]
    pub const fn pushes_nothing(self) -> bool {
        matches!(
            self,
            Self::BadRespawnPoint
                | Self::Cactus
                | Self::Campfire
                | Self::Cramming
                | Self::DragonBreath
                | Self::Drown
                | Self::DryOut
                | Self::EnderPearl
                | Self::Explosion
                | Self::Fall
                | Self::FlyIntoWall
                | Self::Freeze
                | Self::Generic
                | Self::GenericKill
                | Self::HotFloor
                | Self::InFire
                | Self::InWall
                | Self::Lava
                | Self::LightningBolt
                | Self::Magic
                | Self::OnFire
                | Self::OutOfWorld
                | Self::OutsideBorder
                | Self::PlayerExplosion
                | Self::Spear
                | Self::Stalagmite
                | Self::Starve
                | Self::SulfurCubeHot
                | Self::SweetBerryBush
                | Self::Wither
        )
    }
    #[doc = r" The number a client speaking `version` reads this kind as, if it knows it at all."]
    #[doc = r""]
    #[doc = r" [`None`] means the kind was added after that version, which is the honest answer:"]
    #[doc = r" there is no number to send."]
    #[must_use]
    pub const fn wire_id(
        self,
        version: ferrumc_net_codec::version::ProtocolVersion,
    ) -> Option<i32> {
        match DAMAGE_TYPE_IDS[version.index()][self as usize] {
            -1 => None,
            id => Some(id),
        }
    }
}
#[doc = r" Where each kind sits in each supported version's damage type registry, or -1 where the"]
#[doc = r" version does not have it."]
const DAMAGE_TYPE_IDS: [[i32; 51usize]; 10usize] = [
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, -1i32, 8i32, 9i32, 10i32, 11i32, 12i32,
        13i32, 14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, -1i32,
        25i32, 26i32, 27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, -1i32, 35i32, 36i32,
        37i32, 38i32, -1i32, 39i32, 40i32, 41i32, 42i32, 43i32, 44i32, 45i32, 46i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, 25i32, 26i32,
        27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, -1i32, 37i32, 38i32,
        39i32, 40i32, -1i32, 41i32, 42i32, 43i32, 44i32, 45i32, 46i32, 47i32, 48i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, 25i32, 26i32,
        27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, -1i32, 37i32, 38i32,
        39i32, 40i32, -1i32, 41i32, 42i32, 43i32, 44i32, 45i32, 46i32, 47i32, 48i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, 25i32, 26i32,
        27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, -1i32, 37i32, 38i32,
        39i32, 40i32, -1i32, 41i32, 42i32, 43i32, 44i32, 45i32, 46i32, 47i32, 48i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, 25i32, 26i32,
        27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, -1i32, 37i32, 38i32,
        39i32, 40i32, -1i32, 41i32, 42i32, 43i32, 44i32, 45i32, 46i32, 47i32, 48i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, 25i32, 26i32,
        27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, -1i32, 37i32, 38i32,
        39i32, 40i32, -1i32, 41i32, 42i32, 43i32, 44i32, 45i32, 46i32, 47i32, 48i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, 25i32, 26i32,
        27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, -1i32, 37i32, 38i32,
        39i32, 40i32, -1i32, 41i32, 42i32, 43i32, 44i32, 45i32, 46i32, 47i32, 48i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, 25i32, 26i32,
        27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32, 39i32,
        40i32, 41i32, -1i32, 42i32, 43i32, 44i32, 45i32, 46i32, 47i32, 48i32, 49i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, 25i32, 26i32,
        27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32, 39i32,
        40i32, 41i32, -1i32, 42i32, 43i32, 44i32, 45i32, 46i32, 47i32, 48i32, 49i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, 25i32, 26i32,
        27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32, 39i32,
        40i32, 41i32, 42i32, 43i32, 44i32, 45i32, 46i32, 47i32, 48i32, 49i32, 50i32,
    ],
];
