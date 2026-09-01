/* A C host calling a Keleusma protection policy compiled to a native object.
 *
 * THE PROBLEM. Every motor drive derates on temperature and trips on
 * overcurrent. The thresholds are tuned per deployment, and the people who most
 * want to change them are furthest from the firmware team. That is where a host
 * would like field-updatable logic and cannot take the risk, because an
 * unbounded loop or an allocation inside a control loop is a safety incident
 * rather than a slow response.
 *
 * WHAT KELEUSMA BUYS. The policy is total, so it terminates by construction, and
 * its memory is statically bounded, so it cannot exhaust the controller. The
 * verifier rejects a policy whose bound cannot be proved, which is the guarantee
 * the firmware team needs before accepting a field-updatable rule at all.
 *
 * THE CONTRACT is policy.h, GENERATED from the module. Offsets are read out of
 * the compiled artefact rather than transcribed, so they cannot drift.
 */
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <stdlib.h>
#include "policy.h"

/* Q-format helpers. The scale is NOT carried in the value: the header states
 * it, exactly as a C header states the contract for any separately compiled
 * procedure. This policy uses Fixed<8>, so one unit is 1/256. */
#define Q 8
static int64_t to_q(double v) { return (int64_t)(v * (double)(1 << Q)); }
static double from_q(int64_t v) { return (double)v / (double)(1 << Q); }

static int64_t rd(const unsigned char *b, int off) {
    int64_t v;
    memcpy(&v, b + off, sizeof v);
    return v;
}
static void wr(unsigned char *b, int off, int64_t v) { memcpy(b + off, &v, sizeof v); }

int main(void) {
    /* The three trailing pointers the entry takes. The private region must be
     * word-aligned, which is why it is declared as int64_t rather than char. */
    unsigned char shared[KEL_SHARED_BYTES];
    int64_t private_region[8];
    int64_t composite_region[64];

    struct { double t0, t1, t2, amps; } cases[] = {
        {  20.0,  25.0,  30.0,  10.0 },   /* all cool, nothing limited      */
        {  75.0,  25.0,  30.0,  10.0 },   /* zone 0 warm, partial derate    */
        {  95.0,  75.0,  30.0,  10.0 },   /* zone 0 hot, zone 1 warm        */
        {  20.0,  25.0,  30.0, 250.0 },   /* overcurrent, trip              */
    };

    for (unsigned i = 0; i < sizeof cases / sizeof cases[0]; i++) {
        memset(shared, 0, sizeof shared);
        memset(private_region, 0, sizeof private_region);
        memset(composite_region, 0, sizeof composite_region);

        wr(shared, KEL_IO_ZONE_TEMP_0_OFFSET, to_q(cases[i].t0));
        wr(shared, KEL_IO_ZONE_TEMP_1_OFFSET, to_q(cases[i].t1));
        wr(shared, KEL_IO_ZONE_TEMP_2_OFFSET, to_q(cases[i].t2));
        wr(shared, KEL_IO_CURRENT_A_OFFSET,   to_q(cases[i].amps));

        KEL_ENTRY(0, 0, shared,
                  (unsigned char *)private_region,
                  (unsigned char *)composite_region);

        printf("temps %5.1f %5.1f %5.1f  current %6.1f  ->  derate %5.1f%% %5.1f%% %5.1f%%  tripped %d  fault %u\n",
               cases[i].t0, cases[i].t1, cases[i].t2, cases[i].amps,
               from_q(rd(shared, KEL_IO_ZONE_DERATE_0_OFFSET)),
               from_q(rd(shared, KEL_IO_ZONE_DERATE_1_OFFSET)),
               from_q(rd(shared, KEL_IO_ZONE_DERATE_2_OFFSET)),
               shared[KEL_IO_TRIPPED_OFFSET],
               shared[KEL_IO_FAULT_OFFSET]);

        /* The raw contract, for the differential that checks this example.
         * The summary above is for a human; the BYTES are what a wrong offset
         * or a wrong width would corrupt without changing the summary at all.
         * Behind an environment variable so the example reads cleanly when a
         * person runs it and the oracle still gets what it needs. */
        if (getenv("KEL_DUMP_RAW")) {
            printf("RAW ");
            for (int j = 0; j < KEL_SHARED_BYTES; j++) printf("%02x", shared[j]);
            printf("\n");
        }
    }
    return 0;
}
