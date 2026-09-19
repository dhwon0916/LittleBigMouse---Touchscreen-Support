using System.Xml.Linq;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Plugins.Persistence;
using Xunit;

namespace LittleBigMouse.DisplayLayout.Tests;

public class TouchMouseIndependentTests
{
    [Fact]
    public void OldSettingsLeaveTouchSeparationDisabled()
    {
        var options = new ILayoutOptions.Design();
        LayoutDtoMapper.Apply(options, new GlobalOptionsDto());
        Assert.False(options.TouchMouseIndependent);
    }

    [Theory]
    [InlineData(true)]
    [InlineData(false)]
    public void SettingSurvivesPersistenceAndReachesWire(bool enabled)
    {
        var options = new ILayoutOptions.Design { TouchMouseIndependent = enabled };
        var saved = LayoutDtoMapper.ToGlobalOptionsDto(options, null);
        var restored = new ILayoutOptions.Design { TouchMouseIndependent = !enabled };
        LayoutDtoMapper.Apply(restored, saved);
        Assert.Equal(enabled, restored.TouchMouseIndependent);
        using var layout = new MonitorsLayout(restored);
        var zones = layout.ComputeZones();
        Assert.Equal(enabled.ToString(),
            XDocument.Parse(zones.Serialize()).Root!.Attribute("TouchMouseIndependent")!.Value);
    }
}
