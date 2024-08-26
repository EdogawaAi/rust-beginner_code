use std::collections::HashMap;
use nix::sys::ptrace;
use nix::sys::signal;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use std::process::{Child, Command};
use std::os::unix::process::CommandExt;
use nix::sys::ptrace::getregs;
use crate::dwarf_data::DwarfData;
use std::mem::size_of;

fn align_addr_to_word(addr: usize) -> usize {
    addr & (-(size_of::<usize>() as isize) as usize)
}



pub enum Status {
    /// Indicates inferior stopped. Contains the signal that stopped the process, as well as the
    /// current instruction pointer that it is stopped at.
    Stopped(signal::Signal, usize),

    /// Indicates inferior exited normally. Contains the exit status code.
    Exited(i32),

    /// Indicates the inferior exited due to a signal. Contains the signal that killed the
    /// process.
    Signaled(signal::Signal),
}

/// This function calls ptrace with PTRACE_TRACEME to enable debugging on a process. You should use
/// pre_exec with Command to call this in the child process.
fn child_traceme() -> Result<(), std::io::Error> {
    ptrace::traceme().or(Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "ptrace TRACEME failed",
    )))
}

pub struct Inferior {
    child: Child,
}

impl Inferior {
    /// Attempts to start a new inferior process. Returns Some(Inferior) if successful, or None if
    /// an error is encountered.
    pub fn new(target: &str, args: &Vec<String>, breakpoints: &mut HashMap<usize, u8>) -> Option<Inferior> {
        // TODO: implement me!
        // println!(
        //     "Inferior::new not implemented! target={}, args={:?}",
        //     target, args
        // );
        // None
        let mut cmd = Command::new(target);

        cmd.args(args);

        unsafe {
            cmd.pre_exec(child_traceme);
        }

        let child = cmd.spawn().ok()?;

        let mut inferior = Inferior {
            child
        };

        // match inferior.wait(None) {
        //     Ok(Status::Stopped(signal::Signal::SIGTRAP, _)) => Some(inferior),
        //     _ => None,
        // }
        // match inferior.wait(None) {
        //     Ok(Status::Stopped(signal::Signal::SIGTRAP, _)) => {
        //         for &bp in breakpoints {
        //             match inferior.write_byte(bp, 0xcc) {
        //                 Ok(_) => continue,
        //                 Err(_) => println!("Invalid breakpoint address {:#x}", bp),
        //             }
        //         }
        //         Some(inferior)
        //     }
        //     _ => None,
        // }
        let bps = breakpoints.clone();
        for bp in bps.keys() {
            match inferior.write_byte(*bp, 0xcc) {
                Ok(ori_instr) => {breakpoints.insert(*bp, ori_instr);}
                Err(_) => println!("Invalid breakpoint address {:#x}", bp),
            }
        }
        Some(inferior)
    }

    pub fn continue_run(&mut self, signal: Option<signal::Signal>, breakpoints: &HashMap<usize, u8>) -> Result<Status, nix::Error> {
       // ptrace::cont(self.pid(), signal)?;
       // self.wait(None)
        let mut regs = ptrace::getregs(self.pid())?;

        let mut rip = regs.rip as usize;

        if let Some(ori_instr) = breakpoints.get(&(rip - 1)) {
            println!("stopped at a breakpoint");

            self.write_byte(rip - 1, *ori_instr).unwrap();

            regs.rip = (rip - 1) as u64;

            ptrace::setregs(self.pid(), regs).unwrap();

            ptrace::step(self.pid(), None).unwrap();

            match self.wait(None).unwrap() {
                Status::Exited(exit_code) => return Ok(Status::Exited(exit_code)),
                Status::Signaled(signal) => return Ok(Status::Signaled(signal)),
                Status::Stopped(_, _) => {
                    // restore 0xcc in the breakpoint location
                    self.write_byte(rip - 1, 0xcc).unwrap();
                }
            }
        }
        ptrace::cont(self.pid(), signal)?;
        self.wait(None)
    }

    pub fn kill(&mut self) {
        self.child.kill().unwrap();
        self.wait(None).unwrap();
        println!("Killing running inferior (pid {})", self.pid())
    }

    pub fn print_backtrace(&self, debug_data: &DwarfData) -> Result<(), nix::Error> {
        // println!("hello world");
        let regs = ptrace::getregs(self.pid())?;

        let mut instruction_ptr = regs.rip as usize;
        let mut base_ptr = regs.rbp as usize;

        loop {
            let _func = debug_data.get_function_from_addr(instruction_ptr);
            let _line = debug_data.get_line_from_addr(instruction_ptr);
            match (&_func, &_line) {
                (None, None) => println!("unknown func (source file not found)"),
                (Some(line), None) => println!("unknown func ({})", line),
                (None, Some(func)) => println!("{} (source file not found)", func),
                (Some(func), Some(line)) => println!("{} ({})", func, line),
            }
            if let Some(func) = _func {
                if func == "main" {
                    break;
                }
            }
            else {
                break;
            }
            instruction_ptr = ptrace::read(self.pid(), (base_ptr + 8) as ptrace::AddressType)? as usize;
            base_ptr = ptrace::read(self.pid(), base_ptr as ptrace::AddressType)? as usize;
        }

        // println!("%rip register: {:#x}", instruction_ptr);

        Ok(())
    }



    /// Returns the pid of this inferior.
    pub fn pid(&self) -> Pid {
        nix::unistd::Pid::from_raw(self.child.id() as i32)
    }

    /// Calls waitpid on this inferior and returns a Status to indicate the state of the process
    /// after the waitpid call.
    pub fn wait(&self, options: Option<WaitPidFlag>) -> Result<Status, nix::Error> {
        Ok(match waitpid(self.pid(), options)? {
            WaitStatus::Exited(_pid, exit_code) => Status::Exited(exit_code),
            WaitStatus::Signaled(_pid, signal, _core_dumped) => Status::Signaled(signal),
            WaitStatus::Stopped(_pid, signal) => {
                let regs = ptrace::getregs(self.pid())?;
                Status::Stopped(signal, regs.rip as usize)
            }
            other => panic!("waitpid returned unexpected status: {:?}", other),
        })
    }

    pub fn write_byte(&mut self, addr: usize, val: u8) -> Result<u8, nix::Error> {
        let aligned_addr = align_addr_to_word(addr);
        let byte_offset = addr - aligned_addr;
        let word = ptrace::read(self.pid(), aligned_addr as ptrace::AddressType)? as u64;
        let orig_byte = (word >> 8 * byte_offset) & 0xff;
        let masked_word = word & !(0xff << 8 * byte_offset);
        let updated_word = masked_word | ((val as u64) << 8 * byte_offset);
        ptrace::write(
            self.pid(),
            aligned_addr as ptrace::AddressType,
            updated_word as *mut std::ffi::c_void,
        )?;
        Ok(orig_byte as u8)
    }
}
