/*
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 *
 * Author: Aspect
 * Copyright (C) 2024 Aspect
 *
 * All rights reserved. Unauthorized copying of this file, via any medium, is strictly prohibited.
 * Violations will be prosecuted to the fullest extent of the law.
 */

/*
 * WARNING: This software is licensed under the GNU General Public License (GPL).
 * Any attempt to use, modify, or distribute this software in violation of the
 * GPL will be met with strict enforcement. Unauthorized use, modification, or
 * distribution of this software is strictly prohibited and will result in legal
 * action. You are hereby notified that any breach of the GPL license terms will
 * be pursued to the fullest extent of the law, including but not limited to
 * claims for damages and injunctive relief. Compliance with the GPL is not only
 * a legal obligation but a matter of principle to uphold the spirit of open source
 * and fair use. Violations undermine the trust and integrity of the community.
 * BE ADVISED: WE WILL DEFEND OUR RIGHTS VIGOROUSLY.
 */

use arrayvec::{ArrayVec, CapacityError};

pub const FIXED_SIZE: usize = 32;

/// A rectangular entity. **Identifier must be unique.**
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Entity {
    /// Identifier must be unique.
    pub id: u32,

    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// A rectangular query region.
#[derive(Debug, Clone)]
pub struct Query {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl From<Entity> for Query {
    fn from(value: Entity) -> Self {
        Self {
            x: value.x,
            y: value.y,
            width: value.width,
            height: value.height,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Entry(ArrayVec<u32, FIXED_SIZE>);

#[derive(Debug, Clone, Default)]
struct Map(ArrayVec<(u32, u32), FIXED_SIZE>);

/// An extremely optimized fixed-size hash table implementation.
#[derive(Debug, Clone)]
pub struct Table<T: Default + Clone> {
    entries: Vec<T>,
}

impl<T: Default + Clone> Table<T> {
    /// Create a new table with `size` entries.
    pub fn new(size: usize) -> Self {
        let cached_entries_capacity = (size * 1000).next_power_of_two() + 1;
        let entries = vec![T::default(); cached_entries_capacity];
        Self { entries }
    }

    /// Get entry number.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    #[inline(always)]
    fn index(&self, idx: u64) -> usize {
        (hash_u64(idx) % self.entries.len() as u64) as usize
    }

    /// Get a mutable reference to an entry from a 2D key.
    #[inline(always)]
    pub fn get_vector_mut(&mut self, x: u32, y: u32) -> &mut T {
        let idx = self.index(vector_hash(x, y));
        unsafe { self.entries.get_unchecked_mut(idx) }
    }

    /// Get a reference to an entry from a 2D key.
    #[inline(always)]
    pub fn get_vector(&self, x: u32, y: u32) -> &T {
        let idx = self.index(vector_hash(x, y));
        unsafe { self.entries.get_unchecked(idx) }
    }

    /// Get a reference to an entry from a scalar key.
    #[inline(always)]
    pub fn get_scalar(&self, s: u32) -> &T {
        let idx = self.index(hash_u64(s as u64));
        unsafe { self.entries.get_unchecked(idx) }
    }

    /// Get a mutable reference to an entry from a scalar key.
    #[inline(always)]
    pub fn get_scalar_mut(&mut self, s: u32) -> &mut T {
        let idx = self.index(hash_u64(s as u64));
        unsafe { self.entries.get_unchecked_mut(idx) }
    }

    /// Clear the table.
    pub fn clear(&mut self) {
        self.entries.fill(T::default());
    }
}

/// Spatial hash grid implementation.
#[derive(Debug, Clone)]
pub struct Grid {
    grid: Table<Entry>,
    maps: Table<Map>,
    shift: u32,
}

impl Grid {
    /// Create a new grid with a fixed bucket size and cell size.
    pub fn new(size: usize, shift: u32) -> Self {
        Self {
            grid: Table::new(size),
            maps: Table::new(size),
            shift,
        }
    }

    /// Get size of internal tables.
    pub fn count(&self) -> usize {
        self.grid.count()
    }

    /// Insert an entity.
    pub fn insert(&mut self, entity: &Entity) -> Result<(), CapacityError<u32>> {
        let sx = entity.x >> self.shift;
        let sy = entity.y >> self.shift;

        let ex = (entity.x + entity.width) >> self.shift;
        let ey = (entity.y + entity.height) >> self.shift;

        let is_ideal = sx == ex && sy == ey;

        let map = self.maps.get_scalar_mut(entity.id);
        for y in sy..=ey {
            for x in sx..=ex {
                let cell = self.grid.get_vector_mut(x, y);
                map.0.push((x, y));
                cell.0.try_push(entity.id | ((is_ideal as u32) << 31))?;
            }
        }

        Ok(())
    }

    /// Delete an entity by ID.
    pub fn delete(&mut self, id: u32) {
        let map = self.maps.get_scalar(id);
        for &(x, y) in map.0.iter() {
            let cell = self.grid.get_vector_mut(x, y);
            let index = cell.0.iter().position(|x| (*x & !(1 << 31)) == id).unwrap();
            cell.0.remove(index);
        }

        self.maps.get_scalar_mut(id).0.clear();
    }

    /// Retrieve entities in a region.
    pub fn query(&self, query: &Query, result: &mut Vec<u32>) {
        let sx = query.x >> self.shift;
        let sy = query.y >> self.shift;

        let ex = (query.x + query.width) >> self.shift;
        let ey = (query.y + query.height) >> self.shift;

        let is_ideal = sx == ex && sy == ey;

        for y in sy..=ey {
            for x in sx..=ex {
                let region = self.grid.get_vector(x, y);
                for id in region.0.iter() {
                    // there CANNOT be duplicates if we are only checking a single cell.
                    // we do not have to deduplicate an ID if it is known to only occupy a single cell.
                    if id & (1 << 31) != 0 || is_ideal {
                        result.push(*id & !(1 << 31));
                    } else {
                        if !result.contains(id) {
                            result.push(*id);
                        }
                    }
                }
            }
        }
    }

    /// Clear the grid. This method is extremely slow since it iterates over every cell. As a faster alternative, just call the `delete(id)` method for each entity in the grid.
    pub fn clear(&mut self) {
        self.grid.clear();
        self.maps.clear();
    }
}

#[inline]
fn vector_hash(x: u32, y: u32) -> u64 {
    ((x as u64) << 32) | y as u64
}

/// Identity hash for now
#[inline]
fn hash_u64(seed: u64) -> u64 {
    seed
}
