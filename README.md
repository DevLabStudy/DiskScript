# DiskScript

Czytelny język do partycjonowania i formatowania dysków. Zamiast pamiętać komendy diskparta
(`create partition`, `format fs=`, `assign letter=`), opisujesz efekt końcowy, a DiskScript
generuje i (opcjonalnie) uruchamia skrypt diskpart.

## Przykład

```
disk 2 {
    table gpt

    partition instalator {
        size 16GB
        format fat32
        label "INSTALATOR"
        letter F
    }

    partition wymienny {
        size rest
        format exfat
        label "Wymienny"
        letter G
    }
}
```

## Użycie

```
cargo run -- examples/usb.dscript            # dry-run: tylko wypisuje skrypt diskpart
cargo run -- examples/usb.dscript --check    # tylko walidacja
cargo run -- examples/usb.dscript --out s.txt
cargo run -- examples/usb.dscript --run      # wykonanie (Windows, administrator, z potwierdzeniem)
cargo test
```

## Jak to działa

`plik .dscript → Lexer → Parser → AST → Validator → Backend (diskpart) → skrypt → diskpart /s`

## Składnia

- `disk <numer> { ... }` – blok dysku; `table gpt|mbr` (domyślnie gpt)
- `partition <nazwa> { ... }` z właściwościami: `size 100GB|512MB|1TB|rest`, `format ntfs|fat32|exfat|refs`, `label "..."`, `letter E`
- komentarze zaczynają się od `#`

## Zabezpieczenia

- domyślnie dry-run, `--run` wymaga wpisania `TAK, SKASUJ`
- dysk 0 zablokowany (chyba że `--allow-disk0`)
- walidacja: powtórzone litery, litera C, `rest` nie na końcu, FAT32 > 32GB, za długa etykieta, MBR > 4 partycje
