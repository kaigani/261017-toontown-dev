# M1-C1.4 scoped closure: provisional block

Product 5c46aa6. C14-R3-02 is not closed as ruled: BatchRequest denies expectRev, although ADR-0015 section 13 and SI-051 explicitly include it in atomic group params. The retained mismatch probe returns arg.invalid (unknown field expectRev), not rev.conflict. Sequential publication and mixed atomic/sequential replay-history probes pass. Final report and regression verification are in progress.
