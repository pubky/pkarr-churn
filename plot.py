import pandas as pd
import seaborn as sns
import matplotlib.pyplot as plt

def main():
    # Read the CSV file containing the eviction times
    df = pd.read_csv("churns.csv")
    
    # Calculate total records and non-churned count
    total_records = len(df)
    non_churned = (df['time_s'] == 0).sum()
    
    # Create a figure for the plot
    plt.figure(figsize=(10, 6))
    
    # Plot a histogram with a KDE overlay
    sns.histplot(df['time_s'], bins=30, kde=True, color="skyblue")
    
    # Add labels and title
    plt.xlabel("Churn Time (seconds)")
    plt.ylabel("Frequency")
    plt.title("Distribution of DHT Record Churn Times")
    
    # Optionally, mark the mean with a vertical dashed line
    mean_time = df['time_s'].mean()
    plt.axvline(mean_time, color='red', linestyle='dashed', linewidth=1, 
                label=f"Mean: {mean_time:.2f} s")
    plt.legend()
    
    # Annotate the plot with total records and count of non-churned records (t=0)
    plt.text(0.70, 0.75,
             f"Total records: {total_records}\nNon-churned (t=0): {non_churned}",
             transform=plt.gca().transAxes,
             bbox=dict(facecolor='white', alpha=0.5))
    
    # Save and show the plot
    plt.tight_layout()
    plt.savefig("churn_distribution.png")
    plt.show()

if __name__ == '__main__':
    main()
