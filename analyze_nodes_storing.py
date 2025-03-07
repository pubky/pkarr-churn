#!/usr/bin/env python3
"""
This script analyzes node storing data from the nodes_storing experiment using a Weibull survival model.
It:
  - Reads a CSV file ("nodes_storing.csv") with columns "node_count" and "timestamp".
  - Computes the observed survival probability as node_count/initial_count.
  - Converts timestamps to relative time (starting at 0).
  - Fits a Weibull survival model: S(t) = exp[-(t/λ)^k] to the data.
  - Plots the observed survival curve along with the Weibull model fit using seaborn styling.
  - Outputs the fitted Weibull parameters.
  - Computes predicted survival probabilities for a value replicated on 1, 2, 4, 10, 20, 40, and 80 nodes 
    for time horizons of 1 hour, 1 day, 2 days, and 1 week.
    
Summary of Findings:
    The Weibull model was fitted with parameters:
        - Scale (λ) ≈ 40,389.75 seconds
        - Shape (k) ≈ 0.705
    A k value under 1 indicates a high early hazard (many nodes churn quickly) that decreases over time.
    Predicted survival probabilities show that replication dramatically improves the chance that at least one copy survives:
    For example, a value stored on one node has about an 83.38% chance to survive 1 hour but only 18.09% for 1 day.
    With replication (e.g., 10, 20, 40, 80 nodes), survival probabilities increase significantly across all time horizons.
"""

import warnings
warnings.filterwarnings("ignore")

import math
import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
import seaborn as sns
from tabulate import tabulate
from scipy.optimize import curve_fit

def weibull_model(t, lambd, k):
    """Weibull survival model: S(t) = exp[-(t/λ)^k]"""
    return np.exp(- (t / lambd)**k)

def main():
    # Set seaborn style for an attractive plot.
    sns.set(style="whitegrid", context="talk", palette="deep")
    
    # ---------------------------
    # Data Preparation
    # ---------------------------
    df = pd.read_csv("nodes_storing_1.csv")
    df["timestamp"] = pd.to_numeric(df["timestamp"], errors="raise")
    df["node_count"] = pd.to_numeric(df["node_count"], errors="raise")
    
    # Assume initial node count is the maximum observed count.
    initial_count = df["node_count"].max()
    df["survival"] = df["node_count"] / initial_count
    df["time"] = df["timestamp"] - df["timestamp"].min()
    
    # Data summary
    data_summary = [
        ["Initial Node Count", f"{initial_count}"],
        ["Start Time (s)", f"{df['time'].min():.0f}"],
        ["End Time (s)", f"{df['time'].max():.0f}"],
        ["Survival at End", f"{df['survival'].iloc[-1]*100:.2f}%"]
    ]
    print(tabulate(data_summary, headers=["Metric", "Value"], tablefmt="pretty"))
    print()
    
    t = df["time"].values
    survival_obs = df["survival"].values
    
    # ---------------------------
    # Weibull Model Fitting
    # ---------------------------
    # Initial guesses: λ ~ mean(t), k = 1.0.
    initial_guess = [np.mean(t), 1.0]
    popt, pcov = curve_fit(weibull_model, t, survival_obs, p0=initial_guess, maxfev=10000)
    lambd_weib, k_weib = popt
    print("Fitted Weibull Model Parameters:")
    print(f"λ (scale) = {lambd_weib:.2f}")
    print(f"k (shape) = {k_weib:.3f}")
    print()
    
    # ---------------------------
    # Plotting the Observed and Fitted Survival Curves
    # ---------------------------
    fig, ax = plt.subplots(figsize=(10, 7))
    
    # Plot observed survival as a step plot.
    ax.step(df["time"], df["survival"], where="post", label="Observed Survival",
            color=sns.color_palette("deep")[0], marker="o", markersize=8)
    
    # Smooth time axis for model fit.
    t_values = np.linspace(0, df["time"].max(), 200)
    weibull_fit = weibull_model(t_values, lambd_weib, k_weib)
    ax.plot(t_values, weibull_fit, linestyle="--", color=sns.color_palette("deep")[1],
            linewidth=3, label="Weibull Fit")
        
    # Annotate the plot with the Weibull model function and the numerical parameters
    model_text = r'$S(t)=\exp\left[-\left(\frac{t}{%.2f}\right)^{%.3f}\right]$' % (lambd_weib, k_weib)
    ax.text(0.25, 0.95, model_text, transform=ax.transAxes,
            fontsize=16, verticalalignment='top', bbox=dict(boxstyle="round,pad=0.3", fc="white", ec="gray", lw=1))
    
    ax.set_xlabel("Time (s) [Relative]")
    ax.set_ylabel("Survival Probability")
    ax.set_title("Nodes Storing Pkarr Packet Survival Analysis (Weibull Model)")
    ax.legend()
    sns.despine(trim=True)
    
    plt.tight_layout()
    plt.savefig("nodes_storing_weibull_survival.png")
    plt.show()
    
    # ---------------------------
    # Predicted Survival for Replicated Storage
    # ---------------------------
    # Define time horizons in seconds.
    horizons = {
        "1 Hour": 60 * 60,
        "1 Day": 24 *60 * 60,
        "2 Days": 2 * 24 *60 * 60,
        "1 Week": 7 * 24 *60 * 60,
        "1 Month": 30 * 24 *60 * 60
        }
    # Replication counts to predict for.
    replication_counts = [1, 2, 4, 10, 20, 50, 100, 1_000]
    
    prediction_table = []
    for n in replication_counts:
        row = [n]
        for label, t_val in horizons.items():
            # Survival probability for one node at time t using the Weibull model.
            p_single = weibull_model(t_val, lambd_weib, k_weib)
            # For n nodes (assuming independent survival), the chance that at least one survives:
            p_multi = 1 - (1 - p_single)**n
            row.append(f"{p_multi*100:.2f}%")
        prediction_table.append(row)
    
    headers = ["Replication Count"] + list(horizons.keys())
    print("Predicted Survival Probabilities for a Value Replicated on Multiple Nodes:")
    print(tabulate(prediction_table, headers=headers, tablefmt="pretty"))

if __name__ == "__main__":
    main()