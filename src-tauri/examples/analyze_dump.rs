use minidump::{Minidump, Module};
use std::fs;

fn main() {
    let dump_path = std::env::args().nth(1).unwrap_or_else(|| {
        r"C:\Users\<user>\AppData\Roaming\com.assistant\crashes\crash.dmp".to_string()
    });

    let data = fs::read(&dump_path).expect("failed to read dump file");
    let dump = Minidump::read(&data[..]).expect("failed to parse minidump");

    println!("=== Minidump Info ===");
    println!("File: {}", dump_path);

    if let Ok(si) = dump.get_stream::<minidump::MinidumpSystemInfo>() {
        println!("OS: {:?}, CPU: {:?}", si.os, si.cpu);
    }

    let mut exc_addr: u64 = 0;
    let mut exc_module_name = String::new();
    let mut exc_rva: u64 = 0;

    if let Ok(exc) = dump.get_stream::<minidump::MinidumpException>() {
        println!("\n--- Exception ---");
        let code = exc.raw.exception_record.exception_code;
        let name = match code {
            0xC0000005 => "STATUS_ACCESS_VIOLATION",
            0xC0000374 => "STATUS_HEAP_CORRUPTION",
            0xC00000FD => "STATUS_STACK_OVERFLOW",
            0xC000001D => "EXCEPTION_ILLEGAL_INSTRUCTION",
            0x40000015 => "STATUS_FATAL_APP_EXIT",
            _ => "UNKNOWN",
        };
        println!("Exception: {} (0x{:08X})", name, code);
        exc_addr = exc.raw.exception_record.exception_address;
        println!("Exception Address: 0x{:016X}", exc_addr);

        if code == 0xC0000005 {
            let num_params = exc.raw.exception_record.number_parameters as usize;
            if num_params >= 2 {
                let access_type = exc.raw.exception_record.exception_information[0];
                let address = exc.raw.exception_record.exception_information[1];
                let access_name = if access_type == 0 {
                    "READ"
                } else if access_type == 1 {
                    "WRITE"
                } else if access_type == 8 {
                    "DEP"
                } else {
                    "UNKNOWN"
                };
                println!("Access: {} at 0x{:016X}", access_name, address);
            }
        }
        println!("Crashing Thread: {}", exc.raw.thread_id);
    }

    if let Ok(modules) = dump.get_stream::<minidump::MinidumpModuleList>() {
        let mods: Vec<_> = modules.iter().collect();
        println!("\n--- Modules ({} loaded) ---", mods.len());

        if exc_addr != 0 {
            for module in &mods {
                let base = module.base_address();
                let end = base + module.size();
                if exc_addr >= base && exc_addr < end {
                    let short_name = std::path::Path::new(&*module.code_file())
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    exc_rva = exc_addr - base;
                    exc_module_name = short_name.clone();
                    println!(">>> CRASH IN: {} + 0x{:X}", short_name, exc_rva);
                    println!("    Base: 0x{:016X}", base);
                    println!("    Full path: {}", &*module.code_file());
                }
            }
        }

        println!("\n--- Key Modules ---");
        for module in &mods {
            let short_name = std::path::Path::new(&*module.code_file())
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if short_name.contains("assistant")
                || short_name == "ntdll.dll"
                || short_name == "kernelbase.dll"
            {
                let base = module.base_address();
                println!(
                    "  {} base=0x{:016X} size=0x{:X}",
                    short_name,
                    base,
                    module.size()
                );
            }
        }
    }

    if !exc_module_name.is_empty() && exc_module_name == "assistant-app.exe" {
        println!("\n--- Disassembly Info ---");
        println!("Crash RVA: 0x{:X}", exc_rva);
        println!("Use dumpbin to disassemble:");
        println!(
            "  dumpbin /disasm /range:0x{:X},0x{:X} assistant-app.exe",
            exc_rva.saturating_sub(0x40),
            exc_rva + 0x40
        );
        println!("\nOr use objdump:");
        println!(
            "  objdump -d --start-address=0x{:X} --stop-address=0x{:X} assistant-app.exe",
            exc_rva.saturating_sub(0x100),
            exc_rva + 0x100
        );
    }

    println!("\n=== Done ===");
}
