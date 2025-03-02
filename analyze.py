#!/usr/bin/env python3
"""
This script analyzes churn data from mainline DHT experiment.
It:
  - Reads a CSV file ("churns.csv") with columns "pubkey" and "time_s".
    (Records with time_s > 0 indicate the observed churn time in seconds,
     while a value of 0 indicates that the record was still active at experiment end.)
  - Treats records with time_s==0 as right-censored at the experiment's end,
    which is assumed to be the maximum observed churn time.
  - Fits an exponential survival model to the data.
  - Computes the half-life (in seconds) and its 95% confidence interval.
  - Computes the hourly survival probability (e.g. "a record has a 75% survival ratio every hour").
  - Plots the Kaplan–Meier survival curve (observed) along with the exponential model fit,
    using seaborn for styling.
  - Outputs model summary information in a neat formatted table.
  
Note:
  In this version of lifelines, the parameter “lambda_” is estimated as the mean lifetime (μ),
  so that the survival function is: S(t)= exp(-t/μ). Therefore:
      half-life = μ * ln(2)
      hourly survival = exp(-3600/μ)
  
Dependencies: pandas, numpy, matplotlib, seaborn, lifelines, tabulate
Install any missing packages via pip, e.g.:
    pip install pandas numpy matplotlib seaborn lifelines tabulate
"""
import warnings
warnings.filterwarnings("ignore")


import math
import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
import seaborn as sns
from tabulate import tabulate

from lifelines import KaplanMeierFitter, ExponentialFitter

def main():
    sns.set(style="whitegrid")

    # Read CSV data
    df = pd.read_csv("churns_good.csv")
    # Ensure time_s is numeric
    df["time_s"] = pd.to_numeric(df["time_s"], errors="raise")

    # durations = time until event (or censoring)
    durations = df["time_s"].values
    # event: 1 if churned (time_s > 0), 0 if still active (time_s == 0)
    events = (df["time_s"] > 0).astype(int).values

    if events.sum() == 0:
        print("No churn events observed; cannot fit model.")
        return

    # For right-censored records (time_s == 0), set duration to the maximum observed churn time.
    censor_time = df.loc[df["time_s"] > 0, "time_s"].max()
    durations = np.where(durations == 0, censor_time, durations)

    # Data check printout
    data_summary = [
        ["Min Duration (s)", f"{durations.min():.0f}"],
        ["Max Duration (s)", f"{durations.max():.0f}"],
        ["Mean Duration (s)", f"{durations.mean():.2f}"],
        ["# Events", f"{events.sum()} / {len(events)}"]
    ]
    print(tabulate(data_summary, headers=["Metric", "Value"], tablefmt="pretty"))
    print()

    # Fit the exponential survival model using lifelines.
    exp_fitter = ExponentialFitter()
    exp_fitter.fit(durations, event_observed=events)

    # In this version, exp_fitter.params_["lambda_"] is the mean lifetime (μ)
    mu = exp_fitter.params_["lambda_"]
    half_life = mu * math.log(2)
    hourly_survival = math.exp(-3600 / mu)

    # Extract confidence intervals from the summary DataFrame:
    summary_df = exp_fitter.summary
    mu_lower = summary_df.loc["lambda_", "coef lower 95%"]
    mu_upper = summary_df.loc["lambda_", "coef upper 95%"]
    half_life_lower = mu_lower * math.log(2)
    half_life_upper = mu_upper * math.log(2)

    # Prepare model results for printing.
    results = [
        ["Mean Lifetime (μ)", f"{mu:.2f} s"],
        ["Half-life", f"{half_life:.2f} s"],
        ["95% CI for Half-life", f"{half_life_lower:.2f} s - {half_life_upper:.2f} s"],
        ["Hourly Survival Probability", f"{hourly_survival*100:.2f}%"]
    ]
    print(tabulate(results, headers=["Metric", "Value"], tablefmt="pretty"))
    print()

    print("Model summary from lifelines:")
    print(exp_fitter.summary)
    print()

    # Plot the Kaplan-Meier survival curve and the exponential fit.
    kmf = KaplanMeierFitter()
    kmf.fit(durations, event_observed=events, label="Observed Survival")

    t_values = np.linspace(0, durations.max(), 100)
    exp_survival = np.exp(-t_values / mu)  # S(t) = exp(-t/μ)

    fig, ax = plt.subplots(figsize=(8, 6))
    kmf.plot(ax=ax, ci_show=True, label="Observed Survival")
    sns.lineplot(x=t_values, y=exp_survival, ax=ax, linestyle="--", color="red", label="Exponential Fit")

    ax.set_xlabel("Time (s)")
    ax.set_ylabel("Survival Probability")
    ax.set_title("DHT Record Survival Analysis")
    plt.legend()
    plt.tight_layout()
    plt.savefig("survival_plot.png")
    plt.show()

if __name__ == "__main__":
    main()
