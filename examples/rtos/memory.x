/* STM32N6x7 AXI SRAM map. The N6 has no on-chip flash; the
 * application is loaded directly into RAM by probe-rs and runs
 * from there. This linker script lays out a contiguous code +
 * data region inside AXISRAM2, sized for the kernel core, three
 * task VMs with their leaked arenas, and a heap large enough for
 * the dynamic allocations inside `alloc`-backed collections.
 *
 * The map mirrors the layout used by embassy-rs's upstream
 * stm32n6 examples. AXISRAM2 is brought up by the boot ROM
 * regardless of BOOT0 position, so the application is loadable
 * in both factory-boot and development-boot modes (though only
 * the development position lets probe-rs take control).
 *
 *   FLEXRAM  0x34000000   400 KB
 *   AXISRAM1 0x34064000   624 KB   (RCC.memenr.axisram1en = 1 needed; off at reset)
 *   AXISRAM2 0x34100000  1024 KB   (enabled by boot ROM)
 *   AXISRAM3 0x34200000   448 KB
 *   AXISRAM4 0x34270000   448 KB
 *   AXISRAM5 0x342E0000   448 KB
 *   AXISRAM6 0x34350000   448 KB
 *   NPURAM   0x343C0000   256 KB
 *   VENCRAM  0x34400000   128 KB
 *
 * The kernel fills AXISRAM2 entirely: 640 KB FLASH for the
 * Keleusma runtime (lexer, parser, type checker, compiler, VM,
 * verifier, plus the kernel core) and 384 KB RAM for the three
 * task arenas plus the heap, stack, and executor state. Sizes
 * tuned after observing that LlffHeap fragmentation defeated a
 * 224-KB heap with three 64-KB arenas; the wider RAM region and
 * the smaller per-task arena (16 KB, configured in the N6 bin)
 * leave comfortable headroom.
 *
 * Current binary footprint: text ~614 KB (FLASH), bss ~70 KB
 * before the heap buffer plus 320 KB heap (RAM). Future
 * iterations may slim the FLASH image by shipping precompiled
 * bytecode and stripping the compile-time pipeline; until then
 * the full pipeline is included to keep parity with the std
 * demonstrator.
 */
MEMORY
{
  FLASH : ORIGIN = 0x34100000, LENGTH = 640K
  RAM   : ORIGIN = 0x341A0000, LENGTH = 384K
}
