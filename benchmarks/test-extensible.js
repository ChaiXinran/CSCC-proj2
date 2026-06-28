// Test: can we add properties to function-constructed objects after many iterations?
function GridNode(x, y) { this.x = x; this.y = y; }
GridNode.prototype.toString = function() { return "[" + this.x + " " + this.y + "]"; };

function cleanNode(node) {
    node.f = 0;
    node.g = 0;
    node.h = 0;
    node.visited = false;
    node.closed = false;
    node.parent = null;
}

// Create many nodes
var N = 100;
var nodes = [];
for (var i = 0; i < N; i++) {
    for (var j = 0; j < N; j++) {
        var node = new GridNode(i, j);
        nodes.push(node);
    }
}

print("nodes created: " + nodes.length);

// Run many iterations (simulate what astar does)
for (var iter = 0; iter < 10000; iter++) {
    for (var k = 0; k < nodes.length; k++) {
        cleanNode(nodes[k]);
    }
    // Touch some nodes
    if (iter % 1000 === 0) {
        print("iter " + iter + " ok, sample f=" + nodes[0].f);
    }
}
print("done - all iterations passed");
