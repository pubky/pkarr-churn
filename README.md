# DHT Churn Experiment

Measure and visualize churn times for Pkarr records in the Mainline DHT.

## 🚀 Overview

1. **Rust Experiment (`main.rs`)**

   - **Publish:** Pushes records to the DHT.
   - **Churn Tracking:** Periodically checks record resolvability; logs churn times to `churns.csv`.

2. **Python Analysis (`analyze.py`)**
   - Reads `churns.csv`
   - Computes key metrics (mean lifetime, half-life, hourly survival probability)
   - Plots the observed survival curve with an exponential model fit (saved as `survival_plot.png`)

## ⚡️ Setup

### Rust

- Install [Rust](https://www.rust-lang.org/tools/install).
- Build & Run:

```
   cargo build --release
   cargo run --release -- --num-records 100 --stop-fraction 0.9 --ttl-s 604800 --sleep-duration-ms 1000 --max-hours 12
   # or simply `cargo run` for defaults
```

### Python

- Install dependencies:

```
   pip install -r requirements.txt
```

- Generate analysis and plot:

```
   python analyze.py
```

## 🔧 Configuration

| Argument              | Default | Description                                    |
| --------------------- | ------- | ---------------------------------------------- |
| `--num-records`       | 100     | Total records to publish                       |
| `--stop-fraction`     | 0.9     | Fraction of churned records to stop            |
| `--max-hours`         | 12      | Max hours to stop collecting data              |
| `--ttl-s`             | 604800  | TTL for each record (seconds)                  |
| `--sleep-duration-ms` | 1000    | Sleep duration between resolves (milliseconds) |

## 📈 Output

- **Churn Data:** `churns.csv`
- **Analysis & Visualization:** `survival_plot.png`  
  Additionally, key metrics (mean lifetime, half-life, hourly survival probability, and 95% CI) are printed to the terminal.

## 💡 Notes

- Results may vary due to network conditions.
- A pkarr record is considered non-resolvable upon the first occurrence of `None`.
- Non-churned records are logged with `time_s = 0` (and treated as right-censored in analysis).

## 📜 License

MIT License. Contributions welcome!
