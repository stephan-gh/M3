/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

use m3::col::{BTreeMap, BTreeSet, String};
use m3::errors::{Code, VerboseError};
use m3::format;

use crate::config::{AppConfig, TileDesc};
use crate::resources::Resources;

pub fn validate(cfg: &AppConfig, res: &Resources) -> Result<(), VerboseError> {
    validate_services(cfg, &BTreeSet::new())?;
    validate_gates(cfg)?;
    validate_tiles(cfg, res)?;
    validate_mods(cfg, res)
}

fn validate_tiles(cfg: &AppConfig, res: &Resources) -> Result<(), VerboseError> {
    for d in cfg.domains() {
        for a in d.apps() {
            validate_tiles(a, res)?;
        }
    }

    for tile in cfg.tiles() {
        if !tile.optional() {
            let available = count_tiles(res, tile);
            if available < tile.count() {
                return Err(VerboseError::new(
                    Code::NotFound,
                    format!(
                        "AppConfig '{}' needs tile type '{}' {} times, but {} are available",
                        cfg.name(),
                        tile.tile_type().0,
                        tile.count(),
                        available
                    ),
                ));
            }
        }
    }

    Ok(())
}

fn count_tiles(res: &Resources, tile: &TileDesc) -> u32 {
    let mut count = 0;
    for i in 0..res.tiles().count() {
        if tile.tile_type().matches(res.tiles().get(i).desc()) {
            count += 1;
        }
    }
    count
}

fn validate_services(cfg: &AppConfig, parent_set: &BTreeSet<String>) -> Result<(), VerboseError> {
    let mut set = BTreeSet::new();
    for d in cfg.domains() {
        for a in d.apps() {
            for serv in a.services() {
                if set.contains(serv.name().global()) {
                    return Err(VerboseError::new(
                        Code::Exists,
                        format!(
                            "config '{}': service '{}' does already exist",
                            a.name(),
                            serv.name().global()
                        ),
                    ));
                }
                set.insert(serv.name().global().clone());
            }
        }
    }

    let mut subset = set.clone();
    for s in parent_set.iter() {
        if !subset.contains(s) {
            subset.insert(s.clone());
        }
    }
    for d in cfg.domains() {
        for a in d.apps() {
            validate_services(a, &subset)?;
        }
    }

    for sess in cfg.sessions() {
        if !set.contains(sess.name().global()) && !parent_set.contains(sess.name().global()) {
            return Err(VerboseError::new(
                Code::NotFound,
                format!(
                    "config '{}': service '{}' does not exist",
                    cfg.name(),
                    sess.name().global()
                ),
            ));
        }
    }

    Ok(())
}

fn validate_gates(cfg: &AppConfig) -> Result<(), VerboseError> {
    let mut map = BTreeMap::new();
    for d in cfg.domains() {
        for a in d.apps() {
            for rgate in a.rgates() {
                if map.contains_key(rgate.name().global()) {
                    return Err(VerboseError::new(
                        Code::Exists,
                        format!(
                            "config '{}': rgate '{}' does already exist",
                            a.name(),
                            rgate.name().global()
                        ),
                    ));
                }
                map.insert(rgate.name().global().clone(), rgate.slots());
            }
        }
    }

    for d in cfg.domains() {
        for a in d.apps() {
            validate_gates(a)?;

            for sgate in a.sgates() {
                match map.get_mut(sgate.name().global()) {
                    Some(s) => {
                        if *s == 0 {
                            return Err(VerboseError::new(
                                Code::NoSpace,
                                format!(
                                    "config '{}': not enough slots in rgate '{}'",
                                    a.name(),
                                    sgate.name().global()
                                ),
                            ));
                        }
                        *s -= 1;
                    },
                    None => {
                        return Err(VerboseError::new(
                            Code::NotFound,
                            format!(
                                "config '{}': rgate '{}' does not exist",
                                a.name(),
                                sgate.name().global()
                            ),
                        ))
                    },
                }
            }
        }
    }

    Ok(())
}

fn validate_mods(cfg: &AppConfig, res: &Resources) -> Result<(), VerboseError> {
    for d in cfg.domains() {
        for a in d.apps() {
            validate_mods(a, res)?;
        }
    }

    for bmod in cfg.mods() {
        if res.mods().find(bmod.name().global()).is_none() {
            return Err(VerboseError::new(
                Code::NotFound,
                format!(
                    "AppConfig '{}' needs non-existing boot module '{}'",
                    cfg.name(),
                    bmod.name().global(),
                ),
            ));
        }
    }

    Ok(())
}
