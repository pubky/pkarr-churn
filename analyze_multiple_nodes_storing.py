#!/usr/bin/env python3
"""
This script aggregates multiple runs from files named "nodes_storing_*.csv",
fits a Weibull survival model for each run, and computes the 95% confidence intervals 
for both the fitted parameters and the predicted survival probabilities using
the Student's t-distribution. For the predictions, the upper bound is capped at 100%.

Additionally:
1. A plot is saved for each file in the /plots directory (named "node_storing_survival_{number}.png").
2. A master plot is created in the root directory that shows all observed survival curves
   (one per file) and the Weibull model fit using the aggregated mean parameters, with the model formula annotated.
3. Two tables are printed: one for predicted survival probabilities (with 95% CI) and one for lower 95% CI only.
4. An optimal configuration is computed for storing Pkarr records with ≥99% availability,
   optimizing the trade-off between replication count and republishing interval.
"""

import warnings
warnings.filterwarnings("ignore")

import os
import glob
import math
import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
import seaborn as sns
from tabulate import tabulate
from scipy.optimize import curve_fit
from scipy.stats import t

def weibull_model(t, lambd, k):
    """Weibull survival model: S(t) = exp[-(t/λ)^k]"""
    return np.exp(- (t / lambd)**k)

def fit_weibull(file_path):
    """Load data from a file, compute survival, and fit the Weibull model.
       Returns: fitted parameters (lambd, k) and the processed DataFrame."""
    df = pd.read_csv(file_path)
    df["timestamp"] = pd.to_numeric(df["timestamp"], errors="raise")
    df["node_count"] = pd.to_numeric(df["node_count"], errors="raise")
    
    # Use maximum observed node_count as the initial count.
    initial_count = df["node_count"].max()
    df["survival"] = df["node_count"] / initial_count
    df["time"] = df["timestamp"] - df["timestamp"].min()
    
    t_vals = df["time"].values
    survival_obs = df["survival"].values
    
    # Initial guess: λ ~ mean(time) and k = 1.0.
    initial_guess = [np.mean(t_vals), 1.0]
    popt, _ = curve_fit(weibull_model, t_vals, survival_obs, p0=initial_guess, maxfev=10000)
    lambd_weib, k_weib = popt
    return lambd_weib, k_weib, df

def compute_predictions(lambd, k, horizons, replication_counts):
    """Compute predicted survival probabilities for given horizons and replication counts.
       Returns a dictionary of predictions for each replication count and horizon."""
    predictions = {}
    for n in replication_counts:
        preds = {}
        for label, t_val in horizons.items():
            # Survival probability for one node at time t.
            p_single = weibull_model(t_val, lambd, k)
            # For n nodes: probability that at least one survives (assuming independence).
            p_multi = 1 - (1 - p_single)**n
            preds[label] = p_multi
        predictions[n] = preds
    return predictions

def main():
    # Set seaborn style for attractive plots.
    sns.set(style="whitegrid", context="talk", palette="deep")
    
    # ---------------------------
    # Load Multiple Runs and Store Data
    # ---------------------------
    files = glob.glob("nodes_storing_*.csv")
    if not files:
        raise FileNotFoundError("No files found matching the pattern 'nodes_storing_*.csv'")
    
    # Create lists to store aggregated parameters and file data.
    lambda_list = []
    k_list = []
    predictions_all = []  # Store (λ, k) for each run.
    all_file_data = []    # Store tuples of (file, df, λ, k) for plotting.
    
    for file in files:
        lambd_weib, k_weib, df = fit_weibull(file)
        lambda_list.append(lambd_weib)
        k_list.append(k_weib)
        predictions_all.append((lambd_weib, k_weib))
        all_file_data.append((file, df, lambd_weib, k_weib))
    
    lambda_array = np.array(lambda_list)
    k_array = np.array(k_list)
    n_samples = len(lambda_array)
    
    # Compute t-based 95% CI for the Weibull parameters.
    t_crit = t.ppf(0.975, df=n_samples - 1)
    
    lambda_mean = np.mean(lambda_array)
    lambda_std = np.std(lambda_array, ddof=1)
    lambda_ci_lower = lambda_mean - t_crit * (lambda_std / np.sqrt(n_samples))
    lambda_ci_upper = lambda_mean + t_crit * (lambda_std / np.sqrt(n_samples))
    
    k_mean = np.mean(k_array)
    k_std = np.std(k_array, ddof=1)
    k_ci_lower = k_mean - t_crit * (k_std / np.sqrt(n_samples))
    k_ci_upper = k_mean + t_crit * (k_std / np.sqrt(n_samples))
    
    summary_table = [
        ["λ (scale)", f"Mean = {lambda_mean:.2f}", f"95% CI = [{lambda_ci_lower:.2f}, {lambda_ci_upper:.2f}]"],
        ["k (shape)", f"Mean = {k_mean:.3f}", f"95% CI = [{k_ci_lower:.3f}, {k_ci_upper:.3f}]"]
    ]
    print("Aggregated Weibull Parameter Estimates (Student's t-based):")
    print(tabulate(summary_table, headers=["Parameter", "Mean", "95% CI"], tablefmt="pretty"))
    print()
    
    # ---------------------------
    # Save Individual Plots for Each File (with formula annotation)
    # ---------------------------
    os.makedirs("plots", exist_ok=True)
    for idx, (file, df, lambd_weib, k_weib) in enumerate(all_file_data, start=1):
        fig, ax = plt.subplots(figsize=(10, 7))
        # Plot observed survival.
        ax.step(df["time"], df["survival"], where="post", label="Observed Survival",
                color=sns.color_palette("deep")[0], marker="o", markersize=6)
        # Plot the fitted Weibull curve.
        t_vals = np.linspace(0, df["time"].max(), 200)
        model_fit = weibull_model(t_vals, lambd_weib, k_weib)
        ax.plot(t_vals, model_fit, linestyle="--", color=sns.color_palette("deep")[1],
                linewidth=3, label="Weibull Fit")
        # Annotate the plot with the Weibull model formula.
        model_text = r'$S(t)=\exp\left[-\left(\frac{t}{%.2f}\right)^{%.3f}\right]$' % (lambd_weib, k_weib)
        ax.text(0.25, 0.95, model_text, transform=ax.transAxes,
                fontsize=14, verticalalignment='top',
                bbox=dict(boxstyle="round,pad=0.3", fc="white", ec="gray", lw=1))
        
        ax.set_xlabel("Time (s) [Relative]")
        ax.set_ylabel("Survival Probability")
        ax.set_title(f"File: {os.path.basename(file)}")
        ax.legend()
        sns.despine(trim=True)
        
        fig.tight_layout()
        fig.savefig(f"plots/node_storing_survival_{idx}.png")
        plt.close(fig)
    
    # ---------------------------
    # Create Master Plot (All Files + Mean Weibull Model with formula)
    # ---------------------------
    fig, ax = plt.subplots(figsize=(10, 7))
    all_times = []
    for (file, df, lambd_weib, k_weib) in all_file_data:
        ax.step(df["time"], df["survival"], where="post", label=os.path.basename(file),
                alpha=0.7, marker="o", markersize=4)
        all_times.extend(df["time"].tolist())
    max_time = max(all_times) if all_times else 0
    t_vals = np.linspace(0, max_time, 200)
    master_fit = weibull_model(t_vals, lambda_mean, k_mean)
    ax.plot(t_vals, master_fit, linestyle="--", color="black",
            linewidth=3, label="Mean Weibull Fit")
    worst_fit = weibull_model(t_vals, lambda_ci_lower, k_ci_upper)
    ax.plot(t_vals, worst_fit, linestyle=":", color="red", linewidth=3, label="Worst-case 95% CI Fit")
    # Annotate the master plot with the aggregated Weibull model formula.
    master_model_text = r'$S(t)=\exp\left[-\left(\frac{t}{%.2f}\right)^{%.3f}\right]$' % (lambda_mean, k_mean)
    ax.text(0.25, 0.95, master_model_text, transform=ax.transAxes,
            fontsize=16, verticalalignment='top',
            bbox=dict(boxstyle="round,pad=0.3", fc="white", ec="gray", lw=1))
    
    ax.set_xlabel("Time (s) [Relative]")
    ax.set_ylabel("Survival Probability")
    ax.set_title("Master Plot: All Files with Mean Weibull Model")
    ax.legend()
    sns.despine(trim=True)
    
    fig.tight_layout()
    fig.savefig("node_storing_survival_master.png")
    plt.close(fig)
    
    # ---------------------------
    # Predicted Survival for Replicated Storage with t-based CI
    # ---------------------------
    horizons = {
        "1 Hour": 60 * 60,
        "1 Day": 24 * 60 * 60,
        "2 Days": 2 * 24 * 60 * 60,
        "1 Week": 7 * 24 * 60 * 60,
        "1 Month": 30 * 24 * 60 * 60
    }
    # Replication counts to predict for.
    replication_counts = [1, 2, 4, 10, 20, 50, 100, 1_000]
    
    # Gather predictions from each run.
    predictions_dict = {n: {label: [] for label in horizons} for n in replication_counts}
    for lambd_val, k_val in predictions_all:
        for n in replication_counts:
            for label, t_val in horizons.items():
                p_single = weibull_model(t_val, lambd_val, k_val)
                p_multi = 1 - (1 - p_single)**n
                predictions_dict[n][label].append(p_multi)
    
    # Compute mean and t-based 95% CI for each prediction.
    prediction_table = []
    for n in replication_counts:
        row = [n]
        for label in horizons:
            p_vals = np.array(predictions_dict[n][label])
            mean_pred = np.mean(p_vals)
            std_pred = np.std(p_vals, ddof=1)
            n_pred = len(p_vals)
            t_crit_pred = t.ppf(0.975, df=n_pred - 1)
            ci_lower = mean_pred - t_crit_pred * (std_pred / np.sqrt(n_pred))
            ci_upper = mean_pred + t_crit_pred * (std_pred / np.sqrt(n_pred))
            # Ensure lower CI is not negative.
            ci_lower = max(0, ci_lower)
            # Cap the upper CI at 1 (i.e., 100%) if it exceeds the bound.
            ci_upper = min(1, ci_upper)
            row.append(f"{mean_pred*100:.2f}% ({ci_lower*100:.2f}-{ci_upper*100:.2f}%)")
        prediction_table.append(row)
    
    headers = ["Replication Count"] + list(horizons.keys())
    print("Predicted Survival Probabilities with Student's t-based 95% CI for Multiple Replications:")
    print(tabulate(prediction_table, headers=headers, tablefmt="pretty"))
    
    # ---------------------------
    # New Table: Lower 95% CI Intervals Only (with negative values capped at 0)
    # ---------------------------
    lower_ci_table = []
    for n in replication_counts:
        row = [n]
        for label in horizons:
            p_vals = np.array(predictions_dict[n][label])
            std_pred = np.std(p_vals, ddof=1)
            mean_pred = np.mean(p_vals)
            n_pred = len(p_vals)
            t_crit_pred = t.ppf(0.975, df=n_pred - 1)
            ci_lower = mean_pred - t_crit_pred * (std_pred / np.sqrt(n_pred))
            # Cap lower CI at 0 if negative.
            ci_lower = max(0, ci_lower)
            row.append(f"{ci_lower*100:.2f}%")
        lower_ci_table.append(row)
    
    print("\nLower 95% CI Intervals for Predicted Survival Probabilities:")
    print(tabulate(lower_ci_table, headers=headers, tablefmt="pretty"))
    
    # ---------------------------
    # Optimal Replication and Republishing Interval Calculation
    # ---------------------------
    # A) One-shot (Non-Steady) Worst-Case Optimization
    # ---------------------------
    # We use the worst-case parameters (λ_lower and k_upper) with a target availability.
    # For the worst-case scenario, we use:
    #   - λ_lower_worst = lower bound of λ (lambda_ci_lower)
    #   - k_upper_worst = upper bound of k (k_ci_upper)
    # The requirement is that the survival probability for n nodes at time t satisfies:
    #   1 - (1 - exp[-(t/λ_lower_worst)^k_upper_worst])^n >= 0.99.
    # Solving for t (in seconds), we get:
    #   t_max(n) = λ_lower_worst * (-ln(1 - 0.01^(1/n)))^(1/k_upper_worst)
    # where 0.01 = (1 - 0.99).
    target_availability = 0.995  # 99% availability
    def t_max(n, lambda_lower, k_upper, target):
        # Solve: 1 - (1 - exp[-(t/λ_lower)^k_upper])^n = target
        # => t_max = λ_lower * (-ln(1 - (1 - target)^(1/n)))^(1/k_upper)
        return lambda_lower * (-np.log(1 - (1 - target)**(1/n)))**(1/k_upper)
    
    nonsteady_optimal_cost = float('inf')
    nonsteady_optimal_n = None
    nonsteady_optimal_t = None
    for n in range(1, 101):
        t_val = t_max(n, lambda_ci_lower, k_ci_upper, target_availability)
        t_val_hours = t_val / 3600.0
        cost = n / t_val_hours
        if cost < nonsteady_optimal_cost:
            nonsteady_optimal_cost = cost
            nonsteady_optimal_n = n
            nonsteady_optimal_t = t_val
    nonsteady_optimal_t_hours = nonsteady_optimal_t / 3600.0

    print(f"\nOptimal Configuration for ≥{target_availability * 100}% Availability (worst-case):")
    print(f"Replication Count: {nonsteady_optimal_n} nodes")
    print(f"Republishing Interval: {nonsteady_optimal_t_hours:.2f} hours")
    print(f"Cost (Replication Count per Hour): {nonsteady_optimal_cost:.3f} nodes/hour")
    
    # ---------------------------
    # B) Steady-State Optimization (with Minimum Batch Size of 5)
    # ---------------------------
    # For steady state we account for accumulating nodes from previous batches.
    # At the worst-case moment (just before the next batch), batches age by T, 2T, etc.
    # We require that the decayed sum of nodes meets the threshold:
    target_steady = 0.995  # for example, 99.5% availability in steady state
    threshold = -math.log(1 - target_steady)
    
    def A_min(T, lambda_lower, k_upper, max_batches=1000):
        total = 0.0
        for j in range(1, max_batches + 1):
            term = math.exp(-((j * T) / lambda_lower) ** k_upper)
            if term < 1e-12:
                break
            total += term
        return total
    
    steady_optimal_cost = float('inf')
    steady_optimal_T = None  # in seconds
    steady_optimal_R = None
    for T in np.linspace(3600, 5*24*3600, 500):  # from 1 hour to 5 days
        A_val = A_min(T, lambda_ci_lower, k_ci_upper)
        if A_val <= 0:
            continue
        R_min = threshold / A_val
        R_min_int = math.ceil(R_min)
        # Enforce minimum batch size of 5 nodes
        if R_min_int < 5:
            R_min_int = 5
        T_hours = T / 3600.0
        cost = R_min_int / T_hours
        if cost < steady_optimal_cost:
            steady_optimal_cost = cost
            steady_optimal_T = T
            steady_optimal_R = R_min_int
    steady_optimal_T_hours = steady_optimal_T / 3600.0

    print(f"\nSteady-State Optimal Configuration for ≥{target_steady * 100}% Availability:")
    print(f"Replication Count (per batch): {steady_optimal_R} nodes")
    print(f"Republishing Interval: {steady_optimal_T_hours:.2f} hours")
    print(f"Cost (Replication Count per Hour): {steady_optimal_cost:.3f} nodes/hour")
    
    # ---------------------------
    # Final Recommendation Summary
    # ---------------------------
    print("\nFinal Recommendation:")
    print(f"Start by publishing an initial batch of {nonsteady_optimal_n} nodes "
          f"(which achieves ≥99% availability with a republishing interval of {nonsteady_optimal_t_hours:.2f} hours).")
    print(f"Then, to maintain a steady state with ≥{target_availability * 100}% availability, "
          f"publish batches of {steady_optimal_R} nodes every {steady_optimal_T_hours:.2f} hours.")
    print("This strategy minimizes the overall replication cost while ensuring high availability.")

if __name__ == "__main__":
    main()
