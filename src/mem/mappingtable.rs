use core::mem::offset_of;

use crate::fs::File;
use crate::mem::{PTEFlags, PageAlign};

use alloc::vec::Vec;

#[derive(Clone)]
pub struct MapInfo {
    pub mapid: isize,
    pub file: Option<File>,
    pub offset: usize,
    pub va: usize,
    pub filesize: usize,
    pub memsize: usize,
    pub flags: PTEFlags,
}

pub struct MappingTable {
    pub list: Vec<MapInfo>,
}

impl MapInfo {
    pub fn new(
        mapid: isize,
        file: Option<File>,
        offset: usize,
        va: usize,
        filesize: usize,
        memsize: usize,
        flags: PTEFlags,
    ) -> Self {
        MapInfo {
            mapid,
            file,
            offset,
            va,
            filesize,
            memsize,
            flags,
        }
    }

    pub fn va_end(&self) -> usize {
        self.va + self.memsize
    }

    pub fn contains(&self, va: usize) -> bool {
        va >= self.va && va < self.va_end().ceil()
    }
}

impl MappingTable {
    pub fn new() -> Self {
        MappingTable { list: Vec::new() } 
    }

    pub fn map(
        &mut self,
        file: File,
        offset: usize,
        va: usize,
        filesize: usize,
        memsize: usize,
        flags: PTEFlags,
    ) -> isize {
        let mapid = self
            .list
            .iter()
            .map(|x| x.mapid)
            .max()
            .unwrap_or(0) + 1; // oh, we can also store a max here
        self.list.push(MapInfo::new(
            mapid,
            Some(file),
            offset,
            va,
            filesize,
            memsize,
            flags,
        ));
        mapid
    }

    pub fn va_range_check(&mut self, l: usize, r: usize) -> bool {
        for mi in self.list.iter() {
            if !(r <= mi.va || l >= mi.va_end().ceil()) {
                return false;
            }
        }
        true
    }

    pub fn get_by_id(&mut self, mapid:isize) -> Option<&mut MapInfo> {
        self.list.iter_mut().find(|x| x.mapid == mapid)
    }    
}