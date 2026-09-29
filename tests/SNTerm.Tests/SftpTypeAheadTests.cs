using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Runtime.ExceptionServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Threading;
using SNTerm.Models;
using SNTerm.Views;

namespace SNTerm.Tests;

// Kiểm tra tính năng gõ chữ cái để nhảy tới file/thư mục trong danh sách SFTP (giống Windows Explorer),
// chạy trên SftpPanel thật với DataContext giả nên không cần kết nối SSH.
public class SftpTypeAheadTests
{
    // SftpPanel chỉ bind theo tên (Items, SelectedItem) nên không cần SftpViewModel thật.
    private sealed class FakeSftpContext : INotifyPropertyChanged
    {
        public ObservableCollection<SftpItem> Items { get; } = new();

        private SftpItem? _selectedItem;
        public SftpItem? SelectedItem
        {
            get => _selectedItem;
            set { _selectedItem = value; PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(SelectedItem))); }
        }

        public event PropertyChangedEventHandler? PropertyChanged;

        public void Load(params string[] dirs)
        {
            Items.Clear();
            Items.Add(new SftpItem { Name = "..", IsDirectory = true, IsParentDirectory = true });
            foreach (var d in dirs) Items.Add(new SftpItem { Name = d, IsDirectory = true });
        }
    }

    // Danh sách giống /etc: đủ dài để phải cuộn (cửa sổ test chỉ cao 200px) và có nhiều mục cùng chữ đầu.
    private static string[] EtcLikeNames()
    {
        var names = new List<string> { "alternatives", "cloud", "cockpit", "cron.d", "cron.daily" };
        for (int i = 0; i < 120; i++) names.Add($"lib{i:000}");
        names.AddRange(new[] { "nginx", "nsswitch.d", "opt", "ssh", "sysconfig" });
        return names.ToArray();
    }

    private static void RunSta(Action body)
    {
        Exception? error = null;
        var thread = new Thread(() =>
        {
            try { body(); }
            catch (Exception ex) { error = ex; }
            finally { Dispatcher.CurrentDispatcher.InvokeShutdown(); }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        thread.Join();
        if (error != null) ExceptionDispatchInfo.Capture(error).Throw();
    }

    private static void WithPanel(Action<FakeSftpContext, ListView, Window> body)
    {
        RunSta(() =>
        {
            var ctx = new FakeSftpContext();
            ctx.Load(EtcLikeNames());

            var panel = new SftpPanel { DataContext = ctx };
            var window = new Window { Content = panel, Width = 500, Height = 200, ShowActivated = true };
            try
            {
                window.Show();
                window.Activate();
                Pump();

                var list = (ListView)panel.FindName("FileListView");
                list.Focus();
                Pump();

                body(ctx, list, window);
            }
            finally
            {
                window.Close();
            }
        });
    }

    private static void Pump() => Dispatcher.CurrentDispatcher.Invoke(() => { }, DispatcherPriority.ApplicationIdle);

    // Chờ có xử lý message: bộ hẹn giờ reset prefix của TextSearch là DispatcherTimer nên Thread.Sleep sẽ không kích hoạt nó.
    private static void Wait(int milliseconds)
    {
        var frame = new DispatcherFrame();
        var timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(milliseconds) };
        timer.Tick += (_, _) => { timer.Stop(); frame.Continue = false; };
        timer.Start();
        Dispatcher.PushFrame(frame);
    }

    private static void Type(ListView list, string text)
    {
        var composition = new TextComposition(InputManager.Current, list, text);
        list.RaiseEvent(new TextCompositionEventArgs(Keyboard.PrimaryDevice, composition)
        {
            RoutedEvent = TextCompositionManager.TextInputEvent
        });
        Pump();
    }

    private static string? SelectedName(ListView list) => (list.SelectedItem as SftpItem)?.Name;

    [Fact]
    public void TypeLetter_SelectsFirstItemStartingWithIt_AndScrollsToIt()
    {
        WithPanel((ctx, list, _) =>
        {
            Type(list, "n");

            Assert.Equal("nginx", SelectedName(list)); // mục đầu tiên bắt đầu bằng 'n', không phải nsswitch.d
            Assert.Equal("nginx", ctx.SelectedItem?.Name);
            Assert.Single(list.SelectedItems);
            // Danh sách ảo hóa: dòng được chọn phải đã được cuộn vào vùng nhìn thấy thì mới có container.
            Assert.NotNull(list.ItemContainerGenerator.ContainerFromItem(list.SelectedItem));
        });
    }

    [Fact]
    public void TypeSeveralLettersQuickly_NarrowsDownByPrefix()
    {
        WithPanel((_, list, _) =>
        {
            Type(list, "n");
            Type(list, "s");

            Assert.Equal("nsswitch.d", SelectedName(list));
        });
    }

    [Fact]
    public void TypeSameLetterRepeatedly_CyclesThroughItemsWithThatLetter()
    {
        WithPanel((_, list, _) =>
        {
            Type(list, "c");
            Assert.Equal("cloud", SelectedName(list));

            Type(list, "c");
            Assert.Equal("cockpit", SelectedName(list));

            Type(list, "c");
            Assert.Equal("cron.d", SelectedName(list));
        });
    }

    [Fact]
    public void TypeLetter_IsCaseInsensitive_AndNoMatchKeepsSelection()
    {
        WithPanel((_, list, _) =>
        {
            Type(list, "N");
            Assert.Equal("nginx", SelectedName(list));

            Type(list, "z"); // "nz" không khớp mục nào
            Assert.Equal("nginx", SelectedName(list));
        });
    }

    [Fact]
    public void TypeDot_SelectsParentDirectoryRow()
    {
        WithPanel((_, list, _) =>
        {
            Type(list, ".");

            Assert.Equal("..", SelectedName(list));
        });
    }

    [Fact]
    public void PrefixResetsAfterPause_SoNextLetterStartsNewSearch()
    {
        WithPanel((_, list, _) =>
        {
            Type(list, "n");
            Assert.Equal("nginx", SelectedName(list));

            Wait(2500); // lâu hơn thời gian chờ tối đa của TextSearch (tối đa ~2s theo cài đặt bàn phím)

            Type(list, "s");
            Assert.Equal("ssh", SelectedName(list)); // không phải "ns" => nsswitch.d
        });
    }

    [Fact]
    public void TypeLetter_StillWorksAfterListIsReloaded()
    {
        WithPanel((ctx, list, window) =>
        {
            // Người dùng đang đứng ở một dòng rồi vào thư mục khác: danh sách bị xóa và nạp lại, dòng có focus bị gỡ khỏi cây giao diện.
            ((ListViewItem)list.ItemContainerGenerator.ContainerFromIndex(1)).Focus();
            Pump();

            ctx.Load("etc", "home", "usr");
            Pump();

            // Cửa sổ test chỉ giữ được focus bàn phím thật khi được kích hoạt (cần desktop tương tác).
            if (window.IsActive) Assert.True(list.IsKeyboardFocusWithin, "Focus phải quay về danh sách để gõ chữ tiếp được.");

            Type(list, "h");
            Assert.Equal("home", SelectedName(list));
        });
    }
}
