namespace DimScreen;

internal sealed class OverlayWindow : Form
{
    private bool clickThrough;

    public OverlayWindow(Screen screen, AppSettings settings)
    {
        clickThrough = settings.ClickThrough;
        FormBorderStyle = FormBorderStyle.None;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.Manual;
        Bounds = screen.Bounds;
        BackColor = settings.TintColor;
        Opacity = settings.Strength / 100d;
        TopMost = true;
    }

    protected override bool ShowWithoutActivation => true;

    protected override CreateParams CreateParams
    {
        get
        {
            var createParams = base.CreateParams;
            createParams.ExStyle |= NativeMethods.WsExToolWindow | NativeMethods.WsExNoActivate;
            if (clickThrough)
            {
                createParams.ExStyle |= NativeMethods.WsExTransparent;
            }

            return createParams;
        }
    }

    protected override void WndProc(ref Message message)
    {
        if (message.Msg == 0x0021)
        {
            message.Result = new nint(NativeMethods.MaNoActivate);
            return;
        }

        base.WndProc(ref message);
    }

    protected override void OnShown(EventArgs e)
    {
        base.OnShown(e);
        KeepOnTop();
    }

    public void UpdateAppearance(AppSettings settings, nint placeBelow = default)
    {
        BackColor = settings.TintColor;
        Opacity = settings.Strength / 100d;
        if (clickThrough != settings.ClickThrough)
        {
            clickThrough = settings.ClickThrough;
            NativeMethods.SetExtendedStyle(Handle, NativeMethods.WsExTransparent, clickThrough);
            var insertAfter = placeBelow == default ? NativeMethods.HwndTopmost : placeBelow;
            NativeMethods.SetWindowPos(Handle, insertAfter, 0, 0, 0, 0,
                NativeMethods.SwpFrameChanged | NativeMethods.SwpNoActivate | NativeMethods.SwpNoMove | NativeMethods.SwpNoSize);
        }

        if (placeBelow != default)
        {
            PlaceBelow(placeBelow);
        }
    }

    public void PlaceBelow(nint handle)
    {
        if (IsHandleCreated && Visible && handle != default)
        {
            NativeMethods.SetWindowPos(Handle, handle, 0, 0, 0, 0,
                NativeMethods.SwpNoActivate | NativeMethods.SwpNoMove | NativeMethods.SwpNoSize | NativeMethods.SwpShowWindow);
        }
    }

    public void KeepOnTop()
    {
        if (IsHandleCreated && Visible)
        {
            NativeMethods.SetWindowPos(Handle, NativeMethods.HwndTopmost, 0, 0, 0, 0,
                NativeMethods.SwpNoActivate | NativeMethods.SwpNoMove | NativeMethods.SwpNoSize | NativeMethods.SwpShowWindow);
        }
    }

}
