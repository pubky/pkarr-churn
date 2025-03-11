#!/usr/bin/env python3
"""
This script aggregates multiple runs from files named "data/run_*.csv",
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
5. Heatmaps are generated for both the non-steady (one-shot) and steady-state cases.
   For the steady-state heatmap the color scale goes from 0 to 2, with a grid spanning republishing times up to 60 hours
   and replication counts (replicates) up to 30. Also, the minimum nodes per batch is now set to 1.
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
            p_single = weibull_model(t_val, lambd, k)
            # For n nodes: probability that at least one survives (assuming independence).
            p_multi = 1 - (1 - p_single)**n
            preds[label] = p_multi
        predictions[n] = preds
    return predictions

def A_min(T, lambda_lower, k_upper, max_batches=1000):
    """
    Computes the decayed contribution of batches at worst-case,
    A_min(T) = sum_{j=1}^{max_batches} exp[-((j*T)/λ_lower)^k_upper]
    """
    total = 0.0
    for j in range(1, max_batches + 1):
        term = math.exp(-((j * T) / lambda_lower)**k_upper)
        if term < 1e-12:
            break
        total += term
    return total

# ------------------------
# Generic Heatmap Plotting Function
# ------------------------
def plot_heatmap_generic(T_grid, R_grid, cost_grid, contour_grid, contour_levels,
                         contour_format, xlabel, ylabel, title, optimal_x, optimal_y,
                         optimal_text, clim, filename):
    """
    Generic function to plot a heatmap with an overlaid contour and an optimal point marker.
    
    Parameters:
      T_grid, R_grid: Meshgrid arrays for x (republishing time in hours) and y (replication count).
      cost_grid: 2D array of cost values.
      contour_grid: 2D array to draw contours (e.g., survival probability or effective availability).
      contour_levels: List of levels for drawing contours.
      contour_format: Function to format contour labels.
      xlabel, ylabel, title: Labels and title for the plot.
      optimal_x, optimal_y: Coordinates of the optimal configuration (x is time in hours, y is replication count).
      optimal_text: Label for the optimal point.
      clim: Tuple for color scale limits.
      filename: File name to save the plot.
    """
    plt.figure(figsize=(12, 8))
    masked_cost = np.ma.masked_invalid(cost_grid)
    mesh = plt.pcolormesh(T_grid, R_grid, masked_cost, shading='auto', cmap="viridis")
    mesh.set_clim(*clim)
    
    cbar = plt.colorbar(mesh)
    cbar.set_label("Cost (Replication Count / Rep. Time [hr])")
    
    plt.xlabel(xlabel)
    plt.ylabel(ylabel)
    plt.title(title)
    
    if contour_grid is not None and contour_levels is not None:
        cs = plt.contour(T_grid, R_grid, contour_grid, levels=contour_levels,
                         colors='black', linewidths=2)
        plt.clabel(cs, inline=True, fontsize=10, fmt=contour_format)
    
    plt.scatter(optimal_x, optimal_y, color='red', edgecolor='black',
                s=100, zorder=10, label=optimal_text)
    plt.text(optimal_x, optimal_y, f"  ({optimal_y}, {optimal_x:.2f}h)", color='red',
             fontsize=10, verticalalignment='bottom')
    plt.legend()
    plt.tight_layout()
    plt.savefig(filename, dpi=300)
    plt.close()

# ------------------------
# Non-Steady (One-shot) Heatmap Function
# ------------------------
def plot_cost_heatmap(lambda_lower, k_upper, target, optimal_n, optimal_t_hours):
    """
    Creates a heatmap for the non-steady (one-shot) configuration.
    
    The x-axis is republishing time (1 to 50 hours) and the y-axis is replication count (1 to 30 nodes).
    Cost is computed as replication count / republishing time if the survival probability meets the target.
    Only cost values between 0.5 and 2 nodes/hour are shown.
    """
    # Define grid.
    T_hours = np.linspace(1, 50, 1000)
    n_values = np.linspace(1, 30, 1000)
    T_grid, n_grid = np.meshgrid(T_hours, n_values)
    T_sec_grid = T_grid * 3600.0  # convert to seconds

    min_color = 0.55
    
    # Compute survival probability for each (n, T).
    p_grid = 1 - (1 - np.exp(- (T_sec_grid / lambda_lower)**k_upper))**n_grid
    # Compute cost if survival probability meets target.
    cost_grid = n_grid / T_grid
    cost_grid = np.where((cost_grid >= min_color) & (cost_grid <= 2), n_grid / T_grid, np.nan)
    
    thresholds = [0.95, 0.99, 0.995, 0.999, 0.9999]
    contour_format = lambda x: f"{x*100:.2f}%"
    
    plot_heatmap_generic(T_grid, n_grid, cost_grid, p_grid, thresholds, contour_format,
                         "Republishing Time (hours)", "Replication Count (nodes)",
                         f"Heatmap of Cost to Achieve ≥99.9% Availability\n(Only {min_color} to 2 nodes/hour Shown)",
                         optimal_t_hours, optimal_n, "Optimal Point",
                         (min_color, 2), "heatmap_cost.png")

# ------------------------
# Steady-State Heatmap Function
# ------------------------
def plot_cost_heatmap_steady(lambda_lower, k_upper, target, optimal_R, optimal_T_hours):
    """
    Creates a heatmap for the steady-state configuration.
    
    The x-axis is republishing time (1 to 60 hours) and the y-axis is replication count (from 1 to 30 nodes).
    Cost is computed as replication count / republishing time when the effective availability meets the threshold.
    The color scale now goes from 0 to 2 nodes/hour.
    """
    threshold = -np.log(1 - target)
    
    # Define grid.
    T_hours = np.linspace(1, 60, 500)
    R_values = np.linspace(1, 30, 500)
    T_grid, R_grid = np.meshgrid(T_hours, R_values)
    T_sec_grid = T_grid * 3600.0  # convert to seconds
    
    # Compute A_min on the grid.
    A_min_vec = np.vectorize(lambda T: A_min(T, lambda_lower, k_upper))
    A_grid = A_min_vec(T_sec_grid)
    
    # Compute effective availability.
    avail_grid = R_grid * A_grid
    condition = avail_grid >= threshold
    cost_grid = np.where(condition, R_grid / T_grid, np.nan)
    cost_grid = np.where((cost_grid >= 0) & (cost_grid <= 2), cost_grid, np.nan)
    
    contour_levels = [threshold]
    contour_format = lambda x: f"Threshold: {target*100:.1f}%"
    
    plot_heatmap_generic(T_grid, R_grid, cost_grid, avail_grid, contour_levels, contour_format,
                         "Republishing Time (hours)", "Replication Count (nodes)",
                         "Heatmap of Cost to Achieve ≥99.9% Steady-State Availability\n(Only 0 to 2 nodes/hour Shown)",
                         optimal_T_hours, optimal_R, "Optimal Steady Point",
                         (0, 2), "heatmap_cost_steady.png")

def main():
    sns.set(style="whitegrid", context="talk", palette="deep")
    
    # ---------------------------
    # Load Multiple Runs and Store Data
    # ---------------------------
    files = glob.glob("data/run_*.csv")
    if not files:
        raise FileNotFoundError("No files found matching the pattern 'data/run_*.csv'")
    
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
    ci = 99  # 95% CI
    ci_fraq = 1 - (1 - ci/100)/2
    t_crit = t.ppf(ci_fraq, df=n_samples - 1)
    
    lambda_mean = np.mean(lambda_array)
    lambda_std = np.std(lambda_array, ddof=1)
    lambda_ci_lower = lambda_mean - t_crit * (lambda_std / np.sqrt(n_samples))
    lambda_ci_upper = lambda_mean + t_crit * (lambda_std / np.sqrt(n_samples))
    
    k_mean = np.mean(k_array)
    k_std = np.std(k_array, ddof=1)
    k_ci_lower = k_mean - t_crit * (k_std / np.sqrt(n_samples))
    k_ci_upper = k_mean + t_crit * (k_std / np.sqrt(n_samples))
    
    summary_table = [
        ["λ (scale)", f"Mean = {lambda_mean:.2f}", f"{ci}% CI = [{lambda_ci_lower:.2f}, {lambda_ci_upper:.2f}]"],
        ["k (shape)", f"Mean = {k_mean:.3f}", f"{ci}% CI = [{k_ci_lower:.3f}, {k_ci_upper:.3f}]"]
    ]
    print("Aggregated Weibull Parameter Estimates (Student's t-based):")
    print(tabulate(summary_table, headers=["Parameter", "Mean", f"{ci}% CI"], tablefmt="pretty"))
    print()
    
    # ---------------------------
    # Save Individual Plots for Each File (with formula annotation)
    # ---------------------------
    os.makedirs("plots", exist_ok=True)
    for idx, (file, df, lambd_weib, k_weib) in enumerate(all_file_data, start=1):
        fig, ax = plt.subplots(figsize=(10, 7))
        ax.step(df["time"], df["survival"], where="post", label="Observed Survival",
                color=sns.color_palette("deep")[0], marker="o", markersize=6)
        t_vals = np.linspace(0, df["time"].max(), 200)
        model_fit = weibull_model(t_vals, lambd_weib, k_weib)
        ax.plot(t_vals, model_fit, linestyle="--", color=sns.color_palette("deep")[1],
                linewidth=3, label="Weibull Fit")
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
                alpha=0.7, marker="o", markersize=3)
        all_times.extend(df["time"].tolist())
    max_time = max(all_times) if all_times else 0
    t_vals = np.linspace(0, max_time, 10000)
    master_fit = weibull_model(t_vals, lambda_mean, k_mean)
    ax.plot(t_vals, master_fit, linestyle="--", color="black",
            linewidth=1.5, label="Mean Weibull Fit")
    worst_fit = weibull_model(t_vals, lambda_ci_lower, k_ci_upper)
    ax.plot(t_vals, worst_fit, linestyle=":", color="red",
            linewidth=1.5, label=f"Worst-case {ci}% CI Fit")
    master_model_text = r'$S(t)=\exp\left[-\left(\frac{t}{%.2f}\right)^{%.3f}\right]$' % (lambda_mean, k_mean)
    ax.text(0.15, 0.95, master_model_text, transform=ax.transAxes,
            fontsize=16, verticalalignment='top',
            bbox=dict(boxstyle="round,pad=0.3", fc="white", ec="gray", lw=1))
    ax.set_xlabel("Time (s) [Relative]")
    ax.set_ylabel("Survival Probability")
    ax.set_title("Master Plot: All Files with Mean Weibull Model")
    ax.legend()
    sns.despine(trim=True)
    fig.tight_layout()
    fig.savefig("node_storing_survival_master.png", dpi=300)
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
    replication_counts = [1, 2, 4, 10, 20, 50, 100, 1_000]
    predictions_dict = {n: {label: [] for label in horizons} for n in replication_counts}
    for lambd_val, k_val in predictions_all:
        for n in replication_counts:
            for label, t_val in horizons.items():
                p_single = weibull_model(t_val, lambd_val, k_val)
                p_multi = 1 - (1 - p_single)**n
                predictions_dict[n][label].append(p_multi)
    
    prediction_table = []
    for n in replication_counts:
        row = [n]
        for label in horizons:
            p_vals = np.array(predictions_dict[n][label])
            mean_pred = np.mean(p_vals)
            std_pred = np.std(p_vals, ddof=1)
            n_pred = len(p_vals)
            t_crit_pred = t.ppf(ci_fraq, df=n_pred - 1)
            ci_lower = mean_pred - t_crit_pred * (std_pred / np.sqrt(n_pred))
            ci_upper = mean_pred + t_crit_pred * (std_pred / np.sqrt(n_pred))
            ci_lower = max(0, ci_lower)
            ci_upper = min(1, ci_upper)
            row.append(f"{mean_pred*100:.2f}% ({ci_lower*100:.2f}-{ci_upper*100:.2f}%)")
        prediction_table.append(row)
    
    headers = ["Replication Count"] + list(horizons.keys())
    print(f"Predicted Survival Probabilities with Student's t-based {ci}% CI for Multiple Replications:")
    print(tabulate(prediction_table, headers=headers, tablefmt="pretty"))
    
    lower_ci_table = []
    for n in replication_counts:
        row = [n]
        for label in horizons:
            p_vals = np.array(predictions_dict[n][label])
            std_pred = np.std(p_vals, ddof=1)
            mean_pred = np.mean(p_vals)
            n_pred = len(p_vals)
            t_crit_pred = t.ppf(ci_fraq, df=n_pred - 1)
            ci_lower = mean_pred - t_crit_pred * (std_pred / np.sqrt(n_pred))
            ci_lower = max(0, ci_lower)
            row.append(f"{ci_lower*100:.2f}%")
        lower_ci_table.append(row)
    
    print(f"\nLower {ci}% CI Intervals for Predicted Survival Probabilities:")
    print(tabulate(lower_ci_table, headers=headers, tablefmt="pretty"))
    
    # ---------------------------
    # Optimal Replication and Republishing Interval Calculation (Non-Steady)
    # ---------------------------
    target_availability = 0.999  # 99.9% target
    def t_max(n, lambda_lower, k_upper, target):
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

    print(f"\nOptimal Configuration for ≥{target_availability * 100}% Availability (non-steady):")
    print(f"Replication Count: {nonsteady_optimal_n} nodes")
    print(f"Republishing Interval: {nonsteady_optimal_t_hours:.2f} hours")
    print(f"Cost (Replication Count per Hour): {nonsteady_optimal_cost:.3f} nodes/hour")
    
    plot_cost_heatmap(lambda_ci_lower, k_ci_upper, target_availability,
                      nonsteady_optimal_n, nonsteady_optimal_t_hours)
    
    # ---------------------------
    # Optimal Replication and Republishing Interval Calculation (Steady-State)
    # ---------------------------
    target_steady = target_availability  # same target for steady state
    threshold = -math.log(1 - target_steady)
    min_nodes = 1  # updated minimum nodes
    
    steady_optimal_cost = float('inf')
    steady_optimal_T = None  # in seconds
    steady_optimal_R = None
    for T in np.linspace(3600, 5*24*3600, 500):  # from 1 hour to 5 days
        A_val = A_min(T, lambda_ci_lower, k_ci_upper)
        if A_val <= 0:
            continue
        R_min = threshold / A_val
        R_min_int = math.ceil(R_min)
        if R_min_int < min_nodes:
            R_min_int = min_nodes
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
    
    plot_cost_heatmap_steady(lambda_ci_lower, k_ci_upper, target_steady,
                             steady_optimal_R, steady_optimal_T_hours)
    
    # ---------------------------
    # Final Recommendation Summary
    # ---------------------------
    print("\nFinal Recommendation:")
    print(f"Start by publishing to an initial batch of {nonsteady_optimal_n} nodes "
          f"(which achieves ≥99.9% availability with a republishing interval of {nonsteady_optimal_t_hours:.2f} hours).")
    print(f"Then, to maintain a steady state with ≥{target_steady * 100}% availability, "
          f"publish to batches of {steady_optimal_R} nodes every {steady_optimal_T_hours:.2f} hours.")
    print("This strategy minimizes the overall replication cost while ensuring high availability.")
    
    A_min_value = A_min(steady_optimal_T, lambda_ci_lower, k_ci_upper)
    lowest_effective = steady_optimal_R * A_min_value
    highest_effective = steady_optimal_R * (1 + A_min_value)
    print(f"\nAt steady state, the effective replication fluctuates between")
    print(f"approximately {lowest_effective:.2f} nodes (just before republishing) and {highest_effective:.2f} nodes (immediately after republishing).")

if __name__ == "__main__":
    main()
