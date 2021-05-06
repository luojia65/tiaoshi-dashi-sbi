OUTPUT_ARCH(riscv)
PROVIDE(_stext = 0x80000000);
ENTRY(_start)

SECTIONS 
{
    .text _stext : {
        *(.text.entry)
        *(.text .text.*)
        _etext = .;
    }

    .rodata : ALIGN(4) {
        _srodata = .;
        *(.rodata .rodata.*)
        . = ALIGN(4);
        _erodata = .;
    }

    .data : ALIGN(4) { 
        _sidata = LOADADDR(.data);
        _sdata = .;
        PROVIDE(__global_pointer$ = . + 0x800);
        *(.sdata .sdata.* .sdata2 .sdata2.*);
        *(.data .data.*)
        . = ALIGN(4);
        _edata = .;
    }

    .bss (NOLOAD) : ALIGN(4) {
        _sbss = .;
        *(.sbss .bss .bss.*)
        . = ALIGN(4);
        _ebss = .;
    }

    /DISCARD/ :
    {
        *(.eh_frame .eh_frame_hdr);
    }
}
