namespace DimScreen;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        Application.SetHighDpiMode(HighDpiMode.PerMonitorV2);
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        var startHidden = args.Contains("--minimized", StringComparer.OrdinalIgnoreCase);
        Application.Run(new DimScreenContext(startHidden));
    }
}
