add-symbol-file .build/debugger/kernel
layout split
set trace-commands on
set logging file .build/gdb.txt
set logging enabled on
target remote localhost:1234
