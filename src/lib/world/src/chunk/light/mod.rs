use bitcode_derive::{Decode, Encode};
use deepsize::DeepSizeOf;

pub mod network;

#[derive(Default, Clone, DeepSizeOf, Encode, Decode)]
pub(crate) enum LightStorage {
    #[default]
    Empty,
    Full,
    Mixed {
        light_data: Box<[u8]>,
    },
}

#[derive(Clone, DeepSizeOf, Encode, Decode)]
pub struct SectionLightData {
    sky_light: LightStorage,
    block_light: LightStorage,
}

impl Default for SectionLightData {
    fn default() -> Self {
        Self {
            sky_light: LightStorage::Full,
            block_light: LightStorage::default(),
        }
    }
}

impl From<Vec<i8>> for LightStorage {
    fn from(data: Vec<i8>) -> Self {
        if data.len() != 2048 {
            Self::Empty
        } else {
            let mut all_on = true;
            let mut all_off = true;

            for b in data.iter() {
                if *b != i8::MAX {
                    all_on = false
                };
                if *b != i8::MIN {
                    all_off = false
                };
            }

            if all_on {
                Self::Full
            } else if all_off {
                Self::Empty
            } else {
                Self::Mixed {
                    light_data: data.into_iter().map(|v| v as u8).collect(),
                }
            }
        }
    }
}

impl SectionLightData {
    pub(crate) fn with_data(sky_light: LightStorage, block_light: LightStorage) -> Self {
        Self {
            sky_light,
            block_light,
        }
    }

    #[inline]
    pub fn contains_sky_light(&self) -> bool {
        self.sky_light.contains_light()
    }

    #[inline]
    pub fn contains_block_light(&self) -> bool {
        self.block_light.contains_light()
    }

    /// The block light at one position in the section.
    #[must_use]
    pub fn block_light(&self, x: u8, y: u8, z: u8) -> u8 {
        self.block_light.get(x, y, z)
    }

    pub fn set_block_light(&mut self, x: u8, y: u8, z: u8, level: u8) {
        self.block_light.set(x, y, z, level);
    }

    /// The sky light at one position in the section.
    #[must_use]
    pub fn sky_light(&self, x: u8, y: u8, z: u8) -> u8 {
        self.sky_light.get(x, y, z)
    }

    pub fn set_sky_light(&mut self, x: u8, y: u8, z: u8, level: u8) {
        self.sky_light.set(x, y, z, level);
    }

    /// Puts a whole section's sky light at one level, in its uniform form.
    ///
    /// A section that is entirely dark or entirely lit is most of a chunk, and saying so in one
    /// go costs nothing where writing a nibble per block spells the section out.
    pub fn fill_sky_light(&mut self, level: u8) {
        self.sky_light = match level {
            0 => LightStorage::Empty,
            15 => LightStorage::Full,
            _ => {
                let byte = level | (level << 4);
                LightStorage::Mixed {
                    light_data: vec![byte; BYTES].into_boxed_slice(),
                }
            }
        };
    }
}

/// Light is kept as one nibble per block, in the order the chunk packet wants them.
const NIBBLES: usize = 4096;
const BYTES: usize = NIBBLES / 2;

/// Where a position inside a section sits in the nibble array.
const fn nibble_index(x: u8, y: u8, z: u8) -> usize {
    ((y as usize) << 8) | ((z as usize) << 4) | (x as usize)
}

impl LightStorage {
    /// The level at one position.
    #[must_use]
    pub fn get(&self, x: u8, y: u8, z: u8) -> u8 {
        match self {
            Self::Empty => 0,
            Self::Full => 15,
            Self::Mixed { light_data } => {
                let index = nibble_index(x, y, z);
                let byte = light_data.get(index / 2).copied().unwrap_or(0);
                if index.is_multiple_of(2) {
                    byte & 0x0F
                } else {
                    byte >> 4
                }
            }
        }
    }

    /// Sets the level at one position, spelling the storage out if it was all one value.
    pub fn set(&mut self, x: u8, y: u8, z: u8, level: u8) {
        if self.get(x, y, z) == level {
            return;
        }
        if !matches!(self, Self::Mixed { .. }) {
            let fill = match self {
                Self::Full => 0xFF,
                _ => 0x00,
            };
            *self = Self::Mixed {
                light_data: vec![fill; BYTES].into_boxed_slice(),
            };
        }
        let Self::Mixed { light_data } = self else {
            return;
        };
        let index = nibble_index(x, y, z);
        let byte = &mut light_data[index / 2];
        if index.is_multiple_of(2) {
            *byte = (*byte & 0xF0) | (level & 0x0F);
        } else {
            *byte = (*byte & 0x0F) | ((level & 0x0F) << 4);
        }
    }

    #[inline]
    pub fn contains_light(&self) -> bool {
        match self {
            LightStorage::Empty => false,
            LightStorage::Full => true,
            LightStorage::Mixed { .. } => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A section that is all one value only spells itself out once something in it differs.
    #[test]
    fn setting_one_value_spells_the_section_out() {
        let mut storage = LightStorage::Full;
        assert_eq!(storage.get(3, 4, 5), 15);

        storage.set(3, 4, 5, 7);
        assert_eq!(storage.get(3, 4, 5), 7);
        assert_eq!(storage.get(3, 4, 6), 15, "the rest keeps what it had");

        // Setting a value to what it already is leaves the storage as it was.
        let mut untouched = LightStorage::Empty;
        untouched.set(0, 0, 0, 0);
        assert!(matches!(untouched, LightStorage::Empty));
    }

    /// Two positions sharing a byte do not overwrite each other.
    #[test]
    fn neighbouring_values_share_a_byte_without_clashing() {
        let mut storage = LightStorage::Empty;
        storage.set(0, 0, 0, 5);
        storage.set(1, 0, 0, 12);
        assert_eq!(storage.get(0, 0, 0), 5);
        assert_eq!(storage.get(1, 0, 0), 12);
    }
}
