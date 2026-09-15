use std::{collections::BTreeMap, fs};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde::Deserialize;

/// One entry of `assets/sound_types.json`: a vanilla `SoundType` (volume, pitch, and the
/// five break/step/place/hit/fall `SoundEvent`s, given as Pumpkin `Sound` enum variant
/// names).
#[derive(Deserialize)]
struct SoundTypeEntry {
    volume: f32,
    pitch: f32,
    #[serde(rename = "break")]
    break_sound: String,
    step: String,
    place: String,
    hit: String,
    fall: String,
}

/// Generates the `SoundType` struct, its ~127 vanilla constants (`SoundType::WOOD`,
/// `SoundType::STONE`, ...), and `sound_type_for_block`, a per-block lookup keyed by
/// [`crate::block::BlockId`] and built from `assets/block_sound_types.json`.
///
/// Both JSON assets were extracted from the decompiled vanilla 26.2 source by a one-off
/// script (not part of this generator) that parsed:
/// - `SoundType.java`'s `public static final SoundType` constants, for
///   `assets/sound_types.json`;
/// - `Blocks.java`'s `BlockBehaviour.Properties.sound(...)` declarations, resolved
///   through `ofFullCopy`/`ofLegacyCopy` property-inheritance chains and through the
///   `BlockSetType`/`WoodType` constructor overrides used by doors, trapdoors, buttons,
///   pressure plates, fence gates and (hanging) signs (their constructors call
///   `properties.sound(type.soundType())`/`type.hangingSignSoundType()`, which
///   unconditionally overwrites whatever `Blocks.java` set), for
///   `assets/block_sound_types.json`.
///
/// If vanilla's block or sound-type data changes, regenerate these two JSON assets from
/// the updated decompiled source (see `comparison/vanilla-src` in the parent repo) rather
/// than editing them by hand; this function only turns already-resolved JSON into Rust.
pub fn build() -> TokenStream {
    let sound_types: BTreeMap<String, SoundTypeEntry> =
        serde_json::from_str(&fs::read_to_string("../../assets/sound_types.json").unwrap())
            .expect("Failed to parse sound_types.json");
    let block_sound_types: BTreeMap<String, String> = serde_json::from_str(
        &fs::read_to_string("../../assets/block_sound_types.json").unwrap(),
    )
    .expect("Failed to parse block_sound_types.json");

    let consts = sound_types
        .iter()
        .map(|(name, entry)| {
            let const_ident = format_ident!("{}", name);
            let volume = entry.volume;
            let pitch = entry.pitch;
            let break_sound = format_ident!("{}", entry.break_sound);
            let step_sound = format_ident!("{}", entry.step);
            let place_sound = format_ident!("{}", entry.place);
            let hit_sound = format_ident!("{}", entry.hit);
            let fall_sound = format_ident!("{}", entry.fall);
            quote! {
                pub const #const_ident: SoundType = SoundType {
                    volume: #volume,
                    pitch: #pitch,
                    break_sound: Sound::#break_sound,
                    step_sound: Sound::#step_sound,
                    place_sound: Sound::#place_sound,
                    hit_sound: Sound::#hit_sound,
                    fall_sound: Sound::#fall_sound,
                };
            }
        })
        .collect::<TokenStream>();

    let match_arms = block_sound_types
        .iter()
        .map(|(block_name, sound_type_name)| {
            let block_ident = format_ident!("{}", block_name.to_uppercase());
            let sound_type_ident = format_ident!("{}", sound_type_name);
            quote! {
                BlockId::#block_ident => SoundType::#sound_type_ident,
            }
        })
        .collect::<TokenStream>();

    quote! {
        use crate::BlockId;
        use crate::sound::Sound;

        /// Vanilla's `net.minecraft.world.level.block.SoundType`: the volume/pitch and the
        /// five `SoundEvent`s (break/step/place/hit/fall) a block uses.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct SoundType {
            pub volume: f32,
            pub pitch: f32,
            pub break_sound: Sound,
            pub step_sound: Sound,
            pub place_sound: Sound,
            pub hit_sound: Sound,
            pub fall_sound: Sound,
        }

        impl SoundType {
            #consts
        }

        /// Returns the vanilla `SoundType` for a block. Falls back to `SoundType::STONE`
        /// -- vanilla's own default for a block whose `Properties` never call
        /// `.sound(...)` (`BlockBehaviour.java:986`) -- for any block id not covered by
        /// the table (there should be none for a block that exists in vanilla Java 26.2;
        /// this only matters for Bedrock-only or otherwise non-vanilla block ids Pumpkin
        /// may register).
        #[must_use]
        pub const fn sound_type_for_block(id: BlockId) -> SoundType {
            match id {
                #match_arms
                _ => SoundType::STONE,
            }
        }
    }
}
