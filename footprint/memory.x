/* Generous regions, so that no configuration fails to link. The figures measured are
   section sizes, which do not depend on these lengths. */
MEMORY
{
  FLASH : ORIGIN = 0x00000000, LENGTH = 4M
  RAM   : ORIGIN = 0x20000000, LENGTH = 1M
}
