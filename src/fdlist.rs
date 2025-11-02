use crate::fs::File;
use alloc::vec::Vec;

pub struct FDInfo {
    pub fd:     isize,
    pub file:   File,
    pub flag:   usize,
}

pub struct FDList {
    pub list: Vec<FDInfo>,
}

impl FDInfo {
    pub fn new(fd: isize, file: File, flag: usize) -> Self {
        FDInfo { fd, file, flag }
    }
}

impl FDList {
    pub fn new() -> Self {
        FDList { list: Vec::new() }
    }

    pub fn open(&mut self, file: File, flag: usize) -> isize {
        let mut fd = 2;
        self
            .list
            .iter()
            .for_each(|x| {
                if fd < x.fd {
                    fd = x.fd;
                }
            });
            fd += 1;
            self.list.push(FDInfo::new(fd, file, flag));
            fd
    }
    pub fn get_by_fd(&mut self, fd:isize) -> Option<&mut FDInfo> {
        self
            .list
            .iter_mut()
            .find(|x| {
                x.fd == fd
            })
    }
}