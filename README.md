# LiteArrow: Columnar File Format And Auxillary Implementation

Better than parquet (immense respect for parquet).
Use cases: OLAP hot storage cold storage local analytics warehouse analytics and on and on
AI dataset storage et al


Arrow-interop, integrates with additional open data tools.

```rust
use std::fs::File;

use arrow_array::record_batch;
use litearrow::{FileReader, FileWriter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let events = record_batch!(
        ("timestamp_ms", Int64, [1735689600000, 1735689601000]),
        ("customer_id", Int64, [1042, 781]),
        ("country", Utf8, ["IL", "US"]),
        ("amount_cents", Int64, [2599, 499])
    )?;

    let mut writer = FileWriter::try_new(File::create("events.ltar")?, events.schema())?;
    writer.write_batch(&events)?;
    writer.finish()?;

    let mut reader = FileReader::try_new(File::open("events.ltar")?)?;
    assert_eq!(reader.read_batch(0)?, events);
    Ok(())
}
```
