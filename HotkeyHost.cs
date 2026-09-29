namespace DimScreen;

internal sealed class HotkeyHost : NativeWindow, IDisposable
{
    private const int HotkeyId = 4129;
    private uint registeredModifiers;
    private uint registeredKey;
    private bool isRegistered;

    public event Action? Pressed;

    public HotkeyHost()
    {
        CreateHandle(new CreateParams { Caption = "dimscreen hotkey" });
    }

    public bool Configure(int modifiers, int key)
    {
        var nextModifiers = (uint)modifiers;
        var nextKey = (uint)key;
        if (isRegistered && nextModifiers == registeredModifiers && nextKey == registeredKey)
        {
            return true;
        }

        if (isRegistered)
        {
            NativeMethods.UnregisterHotKey(Handle, HotkeyId);
            isRegistered = false;
        }

        isRegistered = NativeMethods.RegisterHotKey(Handle, HotkeyId, nextModifiers, nextKey);
        if (isRegistered)
        {
            registeredModifiers = nextModifiers;
            registeredKey = nextKey;
        }

        return isRegistered;
    }

    protected override void WndProc(ref Message message)
    {
        if (message.Msg == NativeMethods.WmHotkey && message.WParam.ToInt32() == HotkeyId)
        {
            Pressed?.Invoke();
        }

        base.WndProc(ref message);
    }

    public void Dispose()
    {
        if (isRegistered)
        {
            NativeMethods.UnregisterHotKey(Handle, HotkeyId);
            isRegistered = false;
        }

        DestroyHandle();
    }
}
