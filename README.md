# DHT Churn Experiment

Measure and visualize churn times (in milliseconds) for Pkarr records in the Mainline DHT.

## 🚀 Overview

1. **Rust Experiment (`main.rs`)**

   - **Publish:** Pushes records to the DHT.
   - **Churn Tracking:** Periodically checks record resolvability; logs churn times (`churns.csv`).

2. **Python Visualization (`plot.py`)**
   - Reads `churns.csv`
   - Plots churn time distribution (`churn_distribution.png`)

## ⚡️ Setup

### Rust

- Install [Rust](https://www.rust-lang.org/tools/install).
- Build & Run:

```bash
  cargo build --release
  cargo run --release -- --num-records 100 --stop-fraction 0.9 --ttl-s 604800 --sleep-duration-ms 1000
  # or simply `cargo run` for defaults
```

### Python

- Install dependencies:

```bash
  pip install -r requirements.txt
```

- Generate plot:

```bash
  python plot.py
```

---

## 🔧 Configuration

| Argument              | Default | Description                           |
| --------------------- | ------- | ------------------------------------- |
| `--num_records`       | 100     | Total records to publish              |
| `--stop_fraction`     | 0.9     | Fraction of churned records to stop   |
| `--ttl_s`             | 604800  | TTL for each record (seconds)         |
| `--sleep_duration_ms` | 1000    | Sleep between resolves (milliseconds) |

## 📈 Output

- **Churn Data:** `churns.csv`
- **Visualization:** `churn_distribution.png`

## 💡 Notes

- Results may vary due to network conditions.
- We determine a pkarr record is not resolvable on the first `None`.
- Non-churned records are logged with `time_s = 0`.

## 📜 License

MIT License. Contributions welcome!
