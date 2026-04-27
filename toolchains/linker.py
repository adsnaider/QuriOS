#!/usr/bin/env python3
import sys
import os

def main():
    args = sys.argv[1:]
    is_baremetal = False
    
    # Check if this is a baremetal rust link
    is_baremetal = "-flavor" in args and "gnu" in args
        
    if is_baremetal:
        new_args = ["ld.lld"]
        skip_next = False
        for arg in args:
            if skip_next:
                skip_next = False
                continue
            if arg == "-flavor":
                skip_next = True
                continue
            # Strip clang-specific or host-specific flags that ld.lld rejects
            if arg.startswith("-fuse-ld="):
                continue
            if arg in ["--as-needed", "--eh-frame-hdr", "--gc-sections", "-Bdynamic", "-Bstatic", "-pie", "-z", "relro", "now", "noexecstack"]:
                continue
            new_args.append(arg)
            
        os.execvp("ld.lld", new_args)
    else:
        # Standard host link
        os.execvp("clang++", ["clang++"] + args)

if __name__ == "__main__":
    main()
