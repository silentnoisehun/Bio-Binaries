# Bio-Binaries

## Mi ez
24 bio-inspirált rendszersegédprogram Rust nyelven (vírusos propagáció, kvantum összefonódás, neurális kapcsolatok, rezonancia mezők).

## Használat
Magas szintű, elosztott rendszerirányítás binaris protokollon (BioMessage) keresztül.

## Bemenet
CLI parancsok, `omega-master` parancsok, binaris adatcsomagok.

## Kimenet
Binaris rendszerállapot, logok, vagy végrehajtott bio-metoforikus művelet.

## Példa
`cargo run --release --bin omega-master -- start --listen 127.0.0.1:8888`

## Függőségek
Rust, Cargo, tokio, bincode, blake3.

## Megjegyzések
Tiszta Rust, 100% binaris protokoll, 24 bio-modul. A `bio-core` részeként működik.
