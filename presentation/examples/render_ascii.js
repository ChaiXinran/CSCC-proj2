console.log("AgentJS ASCII Renderer");
console.log("");

var width = 56;
var height = 24;
var shades = " .:-=+*#%@";

for (var y = 0; y < height; y++) {
    var line = "";
    for (var x = 0; x < width; x++) {
        var nx = (x - width / 2) / 15;
        var ny = (y - height / 2) / 8;
        var distance = Math.sqrt(nx * nx + ny * ny);
        var wave = Math.sin(nx * 4 - distance * 5) * 0.5 + 0.5;
        var intensity = Math.max(0, 1 - distance / 1.7) * wave;
        var index = Math.floor(intensity * (shades.length - 1));
        line += shades[index];
    }
    console.log(line);
}

