use crate::io::{Read, Seek};
use crate::mem::userbuf::{
    __knrl_read_usr_byte_pc, __knrl_read_usr_exit, __knrl_write_usr_byte_pc, __knrl_write_usr_exit,
};
use crate::mem::PageTable;
use crate::thread::{self, current};
use crate::trap::Frame;
use crate::userproc;

use crate::fs::disk::Swap;
use crate::io::SeekFrom::Start;
use crate::mem::pagetable::PTEFlags;
use crate::mem::palloc::UserPool;
use crate::mem::{PageAlign, PhysAddr, PG_SIZE};
use crate::thread::STACK_TOP;
use mem::PG_SHIFT;

use alloc::vec;

use riscv::register::scause::Exception::{self, *};
use riscv::register::sstatus::{self, SPP};

pub const STACK_LIMIT: usize = 0x800000;

pub fn stack_growth_handler(frame: &Frame, addr: usize, user_mode: bool) -> bool {
    let sp = frame.x[2];
    if addr >= STACK_TOP || addr < STACK_TOP - STACK_LIMIT || addr < sp {
        return false;
    } // not in stack / below sp
    let mut current_pt = unsafe { PageTable::effective_pagetable() };
    current_pt.map(
        PhysAddr::from( unsafe { UserPool::alloc_pages(1) } ),
        PageAlign::floor(addr),
        PG_SIZE,
        PTEFlags::V | PTEFlags::R | PTEFlags::W | PTEFlags::U,
    );
    current_pt.activate();
    true
}

pub fn spt_handler(frame: &Frame, va: usize) -> bool {
    let current = current();
    let spt = current.supplementary_pagetable.lock();
    if let Some(mapinfo) = spt.list.iter().find(|m| m.contains(va)).map(|m| m.clone()) {
        let pos = (va - mapinfo.va).floor();
        let mut pt = unsafe { PageTable::effective_pagetable() };
        spt.release();
        let start_va = unsafe { UserPool::alloc_pages(1) as usize };
        let start_pa = PhysAddr::from(start_va);
        let buf = unsafe { (start_va as *mut [u8; PG_SIZE]).as_mut().unwrap() };
        let size = Swap::read_page(pos + mapinfo.offset, &mut buf[..PG_SIZE]);
        buf[size..].fill(0);
        spt.acquire();

        pt.map(
            start_pa,
            va.floor(),
            PG_SIZE,
            mapinfo.flags | PTEFlags::V | PTEFlags::A,
        );
        pt.activate();
        true
    } else {
        false
    }
}

pub fn mmap_handler(frame: &Frame, va: usize) -> bool {
    let current = current();
    let mapping_table = current.mapping_table.lock();
    if let Some(mut mapinfo) = mapping_table.list.iter().find(|m| m.contains(va)).map(|m| m.clone()) {
        let pos = (va - mapinfo.va).floor();
        mapping_table.release();
        mapinfo
            .file
            .as_mut()
            .unwrap()
            .seek(Start(pos + mapinfo.offset))
            .unwrap();

        let mut current_pt = unsafe { PageTable::effective_pagetable() };

        let start_va = unsafe { UserPool::alloc_pages(1) as usize };
        let start_pa = PhysAddr::from(start_va);
        let buf = unsafe { (start_va as *mut [u8; PG_SIZE]).as_mut().unwrap() };
        let limit = (mapinfo.filesize.max(pos) - pos).min(PG_SIZE);

        let size = mapinfo.file.unwrap().read(&mut buf[..limit]).unwrap();
        buf[size..].fill(0);
        mapping_table.acquire();

        current_pt.map(
            start_pa,
            va.floor(),
            PG_SIZE,
            mapinfo.flags | PTEFlags::V | PTEFlags::A,
        );
        current_pt.activate();
        true
    } else {
        false
    }
}

pub fn handler(frame: &mut Frame, fault: Exception, addr: usize) {
    let privilege = frame.sstatus.spp();

    let present = {
        let table = unsafe { PageTable::effective_pagetable() };
        match table.get_pte(addr) {
            Some(entry) => entry.is_valid(),
            None => false,
        }
    };

    unsafe { sstatus::set_sie() };

    kprintln!(
        "Page fault at {:#x}: {} error {} page in {} context.",
        addr,
        if present { "rights" } else { "not present" },
        match fault {
            StorePageFault => "writing",
            LoadPageFault => "reading",
            InstructionPageFault => "fetching instruction",
            _ => panic!("Unknown Page Fault"),
        },
        match privilege {
            SPP::Supervisor => "kernel",
            SPP::User => "user",
        }
    );

    match privilege {
        SPP::Supervisor => {
            let handled = 
                !present && (
                spt_handler(frame, addr)
                || stack_growth_handler(frame, addr, false)
                || mmap_handler(frame, addr));
            if handled {
                kprintln!("Kernel page fault handled.");
                return;
            }
            if frame.sepc == __knrl_read_usr_byte_pc as _ {
                // Failed to read user byte from kernel space when trap in pagefault
                frame.x[11] = 1; // set a1 to non-zero
                frame.sepc = __knrl_read_usr_exit as _;
            } else if frame.sepc == __knrl_write_usr_byte_pc as _ {
                // Failed to write user byte from kernel space when trap in pagefault
                frame.x[11] = 1; // set a1 to non-zero
                frame.sepc = __knrl_write_usr_exit as _;
            } else {
                panic!("Kernel page fault");
            }
        }
        SPP::User => {
            let handled = 
                !present && (
                spt_handler(frame, addr)
                || stack_growth_handler(frame, addr, true)
                || mmap_handler(frame, addr));
            if handled {
                kprintln!("User page fault handled.");
                return;
            }
            kprintln!(
                "User thread {} dying due to page fault.",
                thread::current().name()
            );
            userproc::exit(-1);
        }
    }
}
