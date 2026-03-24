0 BEGIN PGM CYCLE200 MM
1 TOOL CALL 1 Z S1200.0
2 L Z+50 R0 FMAX
3 CYCL DEF 200 DRILLING ~
   Q200=+2                  ;SET-UP CLEARANCE ~
   Q201=-20                 ;DEPTH ~
   Q206=+150                ;FEED RATE FOR PLNGNG ~
   Q202=+5                  ;PLUNGING DEPTH ~
   Q210=+0                  ;DWELL TIME AT TOP ~
   Q203=+0                  ;SURFACE COORDINATE ~
   Q204=+50                 ;2ND SET-UP CLEARANCE ~
   Q211=+0                  ;DWELL TIME AT DEPTH ~
   Q395=+0                  ;DEPTH REFERENCE
4 L X+30 Y+20 FMAX M3
5 CYCL CALL
6 L Z+50 FMAX
7 M30
8 END PGM CYCLE200 MM
