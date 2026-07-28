
const isInBrowser = false;
const jetStreamHostPrint = typeof globalThis.print === "function"
    ? globalThis.print
    : (...args) => globalThis.console.log(...args);
globalThis.print = jetStreamHostPrint;
var console = { log: (...args) => jetStreamHostPrint(...args) };
var document = globalThis.document = {
    getElementById() { return { innerHTML: "" }; }
};
var testList = "cdjs";
var testIterationCount = 1;
var RAMification = false;
var JetStreamParams = {
    prefetchResources: false,
    forceGC: false,
    dumpJSONResults: false,
    testIterationCountMap: {},
    testWorstCaseCountMap: {},
    testList: "cdjs",
};
var __jetstreamResources = {"./cdjs/constants.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\r\n// Copyright (C) 2015 Apple Inc. All rights reserved.\r\n// \r\n// Redistribution and use in source and binary forms, with or without\r\n// modification, are permitted provided that the following conditions are met:\r\n//  * Redistributions of source code must retain the above copyright\r\n//    notice, this list of conditions and the following disclaimer.\r\n//  * Redistributions in binary form must reproduce the above copyright\r\n//    notice, this list of conditions and the following disclaimer in the\r\n//    documentation and/or other materials provided with the distribution.\r\n//  * Neither the name of the Purdue University nor the\r\n//    names of its contributors may be used to endorse or promote products\r\n//    derived from this software without specific prior written permission.\r\n// \r\n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\r\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\r\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\r\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\r\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\r\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\r\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\r\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\r\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\r\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\r\n\r\nvar Constants = {};\r\nConstants.MIN_X = 0;\r\nConstants.MIN_Y = 0;\r\nConstants.MAX_X = 1000;\r\nConstants.MAX_Y = 1000;\r\nConstants.MIN_Z = 0;\r\nConstants.MAX_Z = 10;\r\nConstants.PROXIMITY_RADIUS = 1;\r\nConstants.GOOD_VOXEL_SIZE = Constants.PROXIMITY_RADIUS * 2;\r\n","./cdjs/util.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\r\n// Copyright (C) 2015 Apple Inc. All rights reserved.\r\n// \r\n// Redistribution and use in source and binary forms, with or without\r\n// modification, are permitted provided that the following conditions are met:\r\n//  * Redistributions of source code must retain the above copyright\r\n//    notice, this list of conditions and the following disclaimer.\r\n//  * Redistributions in binary form must reproduce the above copyright\r\n//    notice, this list of conditions and the following disclaimer in the\r\n//    documentation and/or other materials provided with the distribution.\r\n//  * Neither the name of the Purdue University nor the\r\n//    names of its contributors may be used to endorse or promote products\r\n//    derived from this software without specific prior written permission.\r\n// \r\n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\r\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\r\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\r\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\r\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\r\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\r\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\r\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\r\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\r\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\r\n\r\nfunction compareNumbers(a, b) {\r\n    if (a == b)\r\n        return 0;\r\n    if (a < b)\r\n        return -1;\r\n    if (a > b)\r\n        return 1;\r\n    \r\n    // We say that NaN is smaller than non-NaN.\r\n    if (a == a)\r\n        return 1;\r\n    return -1;\r\n}\r\n\r\nfunction averageAbovePercentile(numbers, percentile) {\r\n    // Don't change the original array.\r\n    numbers = numbers.slice();\r\n    \r\n    // Sort in ascending order.\r\n    numbers.sort(function(a, b) { return a - b; });\r\n    \r\n    // Now the elements we want are at the end. Keep removing them until the array size shrinks too much.\r\n    // Examples assuming percentile = 99:\r\n    //\r\n    // - numbers.length starts at 100: we will remove just the worst entry and then not remove anymore,\r\n    //   since then numbers.length / originalLength = 0.99.\r\n    //\r\n    // - numbers.length starts at 1000: we will remove the ten worst.\r\n    //\r\n    // - numbers.length starts at 10: we will remove just the worst.\r\n    var numbersWeWant = [];\r\n    var originalLength = numbers.length;\r\n    while (numbers.length / originalLength > percentile / 100)\r\n        numbersWeWant.push(numbers.pop());\r\n    \r\n    var sum = 0;\r\n    for (var i = 0; i < numbersWeWant.length; ++i)\r\n        sum += numbersWeWant[i];\r\n    \r\n    var result = sum / numbersWeWant.length;\r\n    \r\n    // Do a sanity check.\r\n    if (numbers.length && result < numbers[numbers.length - 1]) {\r\n        throw \"Sanity check fail: the worst case result is \" + result +\r\n            \" but we didn't take into account \" + numbers;\r\n    }\r\n    \r\n    return result;\r\n}\r\n\r\nvar currentTime;\r\nif (this.performance && performance.now)\r\n    currentTime = function() { return performance.now() };\r\nelse if (preciseTime)\r\n    currentTime = function() { return preciseTime() * 1000; };\r\nelse\r\n    currentTime = function() { return 0 + new Date(); };\r\n","./cdjs/red_black_tree.js":"/*\r\n * Copyright (C) 2010, 2011, 2015 Apple Inc. All rights reserved.\r\n *\r\n * Redistribution and use in source and binary forms, with or without\r\n * modification, are permitted provided that the following conditions\r\n * are met:\r\n *\r\n * 1.  Redistributions of source code must retain the above copyright\r\n *     notice, this list of conditions and the following disclaimer.\r\n * 2.  Redistributions in binary form must reproduce the above copyright\r\n *     notice, this list of conditions and the following disclaimer in the\r\n *     documentation and/or other materials provided with the distribution.\r\n * 3.  Neither the name of Apple Inc. (\"Apple\") nor the names of\r\n *     its contributors may be used to endorse or promote products derived\r\n *     from this software without specific prior written permission.\r\n *\r\n * THIS SOFTWARE IS PROVIDED BY APPLE AND ITS CONTRIBUTORS \"AS IS\" AND ANY\r\n * EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\r\n * WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\r\n * DISCLAIMED. IN NO EVENT SHALL APPLE OR ITS CONTRIBUTORS BE LIABLE FOR ANY\r\n * DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\r\n * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\r\n * LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\r\n * ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\r\n * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF\r\n * THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\r\n */\r\n\r\nvar RedBlackTree = (function(){\r\n    function compare(a, b) {\r\n        return a.compareTo(b);\r\n    }\r\n    \r\n    function treeMinimum(x) {\r\n        while (x.left)\r\n            x = x.left;\r\n        return x;\r\n    }\r\n    \r\n    function treeMaximum(x) {\r\n        while (x.right)\r\n            x = x.right;\r\n        return x;\r\n    }\r\n    \r\n    function Node(key, value) {\r\n        this.key = key;\r\n        this.value = value;\r\n        this.left = null;\r\n        this.right = null;\r\n        this.parent = null;\r\n        this.color = \"red\";\r\n    }\r\n    \r\n    Node.prototype.successor = function() {\r\n        var x = this;\r\n        if (x.right)\r\n            return treeMinimum(x.right);\r\n        var y = x.parent;\r\n        while (y && x == y.right) {\r\n            x = y;\r\n            y = y.parent;\r\n        }\r\n        return y;\r\n    };\r\n    \r\n    Node.prototype.predecessor = function() {\r\n        var x = this;\r\n        if (x.left)\r\n            return treeMaximum(x.left);\r\n        var y = x.parent;\r\n        while (y && x == y.left) {\r\n            x = y;\r\n            y = y.parent;\r\n        }\r\n        return y;\r\n    };\r\n    \r\n    Node.prototype.toString = function() {\r\n        return this.key + \"=>\" + this.value + \" (\" + this.color + \")\";\r\n    };\r\n    \r\n    function RedBlackTree() {\r\n        this._root = null;\r\n    }\r\n    \r\n    RedBlackTree.prototype.put = function(key, value) {\r\n        var insertionResult = this._treeInsert(key, value);\r\n        if (!insertionResult.isNewEntry)\r\n            return insertionResult.oldValue;\r\n        var x = insertionResult.newNode;\r\n        \r\n        while (x != this._root && x.parent.color == \"red\") {\r\n            if (x.parent == x.parent.parent.left) {\r\n                var y = x.parent.parent.right;\r\n                if (y && y.color == \"red\") {\r\n                    // Case 1\r\n                    x.parent.color = \"black\";\r\n                    y.color = \"black\";\r\n                    x.parent.parent.color = \"red\";\r\n                    x = x.parent.parent;\r\n                } else {\r\n                    if (x == x.parent.right) {\r\n                        // Case 2\r\n                        x = x.parent;\r\n                        this._leftRotate(x);\r\n                    }\r\n                    // Case 3\r\n                    x.parent.color = \"black\";\r\n                    x.parent.parent.color = \"red\";\r\n                    this._rightRotate(x.parent.parent);\r\n                }\r\n            } else {\r\n                // Same as \"then\" clause with \"right\" and \"left\" exchanged.\r\n                var y = x.parent.parent.left;\r\n                if (y && y.color == \"red\") {\r\n                    // Case 1\r\n                    x.parent.color = \"black\";\r\n                    y.color = \"black\";\r\n                    x.parent.parent.color = \"red\";\r\n                    x = x.parent.parent;\r\n                } else {\r\n                    if (x == x.parent.left) {\r\n                        // Case 2\r\n                        x = x.parent;\r\n                        this._rightRotate(x);\r\n                    }\r\n                    // Case 3\r\n                    x.parent.color = \"black\";\r\n                    x.parent.parent.color = \"red\";\r\n                    this._leftRotate(x.parent.parent);\r\n                }\r\n            }\r\n        }\r\n        \r\n        this._root.color = \"black\";\r\n        return null;\r\n    };\r\n    \r\n    RedBlackTree.prototype.remove = function(key) {\r\n        var z = this._findNode(key);\r\n        if (!z)\r\n            return null;\r\n        \r\n        // Y is the node to be unlinked from the tree.\r\n        var y;\r\n        if (!z.left || !z.right)\r\n            y = z;\r\n        else\r\n            y = z.successor();\r\n\r\n        // Y is guaranteed to be non-null at this point.\r\n        var x;\r\n        if (y.left)\r\n            x = y.left;\r\n        else\r\n            x = y.right;\r\n        \r\n        // X is the child of y which might potentially replace y in the tree. X might be null at\r\n        // this point.\r\n        var xParent;\r\n        if (x) {\r\n            x.parent = y.parent;\r\n            xParent = x.parent;\r\n        } else\r\n            xParent = y.parent;\r\n        if (!y.parent)\r\n            this._root = x;\r\n        else {\r\n            if (y == y.parent.left)\r\n                y.parent.left = x;\r\n            else\r\n                y.parent.right = x;\r\n        }\r\n        \r\n        if (y != z) {\r\n            if (y.color == \"black\")\r\n                this._removeFixup(x, xParent);\r\n            \r\n            y.parent = z.parent;\r\n            y.color = z.color;\r\n            y.left = z.left;\r\n            y.right = z.right;\r\n            \r\n            if (z.left)\r\n                z.left.parent = y;\r\n            if (z.right)\r\n                z.right.parent = y;\r\n            if (z.parent) {\r\n                if (z.parent.left == z)\r\n                    z.parent.left = y;\r\n                else\r\n                    z.parent.right = y;\r\n            } else\r\n                this._root = y;\r\n        } else if (y.color == \"black\")\r\n            this._removeFixup(x, xParent);\r\n        \r\n        return z.value;\r\n    };\r\n    \r\n    RedBlackTree.prototype.get = function(key) {\r\n        var node = this._findNode(key);\r\n        if (!node)\r\n            return null;\r\n        return node.value;\r\n    };\r\n    \r\n    RedBlackTree.prototype.forEach = function(callback) {\r\n        if (!this._root)\r\n            return;\r\n        for (var current = treeMinimum(this._root); current; current = current.successor())\r\n            callback(current.key, current.value);\r\n    };\r\n    \r\n    RedBlackTree.prototype.asArray = function() {\r\n        var result = [];\r\n        this.forEach(function(key, value) {\r\n            result.push({key: key, value: value});\r\n        });\r\n        return result;\r\n    };\r\n    \r\n    RedBlackTree.prototype.toString = function() {\r\n        var result = \"[\";\r\n        var first = true;\r\n        this.forEach(function(key, value) {\r\n            if (first)\r\n                first = false;\r\n            else\r\n                result += \", \";\r\n            result += key + \"=>\" + value;\r\n        });\r\n        return result + \"]\";\r\n    };\r\n    \r\n    RedBlackTree.prototype._findNode = function(key) {\r\n        for (var current = this._root; current;) {\r\n            var comparisonResult = compare(key, current.key);\r\n            if (!comparisonResult)\r\n                return current;\r\n            if (comparisonResult < 0)\r\n                current = current.left;\r\n            else\r\n                current = current.right;\r\n        }\r\n        return null;\r\n    };\r\n    \r\n    RedBlackTree.prototype._treeInsert = function(key, value) {\r\n        var y = null;\r\n        var x = this._root;\r\n        while (x) {\r\n            y = x;\r\n            var comparisonResult = key.compareTo(x.key);\r\n            if (comparisonResult < 0)\r\n                x = x.left;\r\n            else if (comparisonResult > 0)\r\n                x = x.right;\r\n            else {\r\n                var oldValue = x.value;\r\n                x.value = value;\r\n                return {isNewEntry:false, oldValue:oldValue};\r\n            }\r\n        }\r\n        var z = new Node(key, value);\r\n        z.parent = y;\r\n        if (!y)\r\n            this._root = z;\r\n        else {\r\n            if (key.compareTo(y.key) < 0)\r\n                y.left = z;\r\n            else\r\n                y.right = z;\r\n        }\r\n        return {isNewEntry:true, newNode:z};\r\n    };\r\n    \r\n    RedBlackTree.prototype._leftRotate = function(x) {\r\n        var y = x.right;\r\n        \r\n        // Turn y's left subtree into x's right subtree.\r\n        x.right = y.left;\r\n        if (y.left)\r\n            y.left.parent = x;\r\n        \r\n        // Link x's parent to y.\r\n        y.parent = x.parent;\r\n        if (!x.parent)\r\n            this._root = y;\r\n        else {\r\n            if (x == x.parent.left)\r\n                x.parent.left = y;\r\n            else\r\n                x.parent.right = y;\r\n        }\r\n        \r\n        // Put x on y's left.\r\n        y.left = x;\r\n        x.parent = y;\r\n        \r\n        return y;\r\n    };\r\n    \r\n    RedBlackTree.prototype._rightRotate = function(y) {\r\n        var x = y.left;\r\n        \r\n        // Turn x's right subtree into y's left subtree.\r\n        y.left = x.right;\r\n        if (x.right)\r\n            x.right.parent = y;\r\n        \r\n        // Link y's parent to x;\r\n        x.parent = y.parent;\r\n        if (!y.parent)\r\n            this._root = x;\r\n        else {\r\n            if (y == y.parent.left)\r\n                y.parent.left = x;\r\n            else\r\n                y.parent.right = x;\r\n        }\r\n        \r\n        x.right = y;\r\n        y.parent = x;\r\n        \r\n        return x;\r\n    };\r\n    \r\n    RedBlackTree.prototype._removeFixup = function(x, xParent) {\r\n        while (x != this._root && (!x || x.color == \"black\")) {\r\n            if (x == xParent.left) {\r\n                // Note: the text points out that w cannot be null. The reason is not obvious from\r\n                // simply looking at the code; it comes about from the properties of the red-black\r\n                // tree.\r\n                var w = xParent.right;\r\n                if (w.color == \"red\") {\r\n                    // Case 1\r\n                    w.color = \"black\";\r\n                    xParent.color = \"red\";\r\n                    this._leftRotate(xParent);\r\n                    w = xParent.right;\r\n                }\r\n                if ((!w.left || w.left.color == \"black\")\r\n                    && (!w.right || w.right.color == \"black\")) {\r\n                    // Case 2\r\n                    w.color = \"red\";\r\n                    x = xParent;\r\n                    xParent = x.parent;\r\n                } else {\r\n                    if (!w.right || w.right.color == \"black\") {\r\n                        // Case 3\r\n                        w.left.color = \"black\";\r\n                        w.color = \"red\";\r\n                        this._rightRotate(w);\r\n                        w = xParent.right;\r\n                    }\r\n                    // Case 4\r\n                    w.color = xParent.color;\r\n                    xParent.color = \"black\";\r\n                    if (w.right)\r\n                        w.right.color = \"black\";\r\n                    this._leftRotate(xParent);\r\n                    x = this._root;\r\n                    xParent = x.parent;\r\n                }\r\n            } else {\r\n                // Same as \"then\" clause with \"right\" and \"left\" exchanged.\r\n                \r\n                var w = xParent.left;\r\n                if (w.color == \"red\") {\r\n                    // Case 1\r\n                    w.color = \"black\";\r\n                    xParent.color = \"red\";\r\n                    this._rightRotate(xParent);\r\n                    w = xParent.left;\r\n                }\r\n                if ((!w.right || w.right.color == \"black\")\r\n                    && (!w.left || w.left.color == \"black\")) {\r\n                    // Case 2\r\n                    w.color = \"red\";\r\n                    x = xParent;\r\n                    xParent = x.parent;\r\n                } else {\r\n                    if (!w.left || w.left.color == \"black\") {\r\n                        // Case 3\r\n                        w.right.color = \"black\";\r\n                        w.color = \"red\";\r\n                        this._leftRotate(w);\r\n                        w = xParent.left;\r\n                    }\r\n                    // Case 4\r\n                    w.color = xParent.color;\r\n                    xParent.color = \"black\";\r\n                    if (w.left)\r\n                        w.left.color = \"black\";\r\n                    this._rightRotate(xParent);\r\n                    x = this._root;\r\n                    xParent = x.parent;\r\n                }\r\n            }\r\n        }\r\n        if (x)\r\n            x.color = \"black\";\r\n    };\r\n    \r\n    return RedBlackTree;\r\n})();\r\n\r\n","./cdjs/call_sign.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\r\n// Copyright (C) 2015 Apple Inc. All rights reserved.\r\n// \r\n// Redistribution and use in source and binary forms, with or without\r\n// modification, are permitted provided that the following conditions are met:\r\n//  * Redistributions of source code must retain the above copyright\r\n//    notice, this list of conditions and the following disclaimer.\r\n//  * Redistributions in binary form must reproduce the above copyright\r\n//    notice, this list of conditions and the following disclaimer in the\r\n//    documentation and/or other materials provided with the distribution.\r\n//  * Neither the name of the Purdue University nor the\r\n//    names of its contributors may be used to endorse or promote products\r\n//    derived from this software without specific prior written permission.\r\n// \r\n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\r\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\r\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\r\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\r\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\r\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\r\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\r\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\r\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\r\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\r\n\r\nfunction CallSign(value) {\r\n    this._value = value;\r\n}\r\n\r\nCallSign.prototype.compareTo = function(other) {\r\n    return this._value.localeCompare(other._value);\r\n}\r\n\r\nCallSign.prototype.toString = function() {\r\n    return this._value;\r\n}\r\n\r\n","./cdjs/vector_2d.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\r\n// Copyright (C) 2015 Apple Inc. All rights reserved.\r\n// \r\n// Redistribution and use in source and binary forms, with or without\r\n// modification, are permitted provided that the following conditions are met:\r\n//  * Redistributions of source code must retain the above copyright\r\n//    notice, this list of conditions and the following disclaimer.\r\n//  * Redistributions in binary form must reproduce the above copyright\r\n//    notice, this list of conditions and the following disclaimer in the\r\n//    documentation and/or other materials provided with the distribution.\r\n//  * Neither the name of the Purdue University nor the\r\n//    names of its contributors may be used to endorse or promote products\r\n//    derived from this software without specific prior written permission.\r\n// \r\n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\r\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\r\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\r\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\r\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\r\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\r\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\r\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\r\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\r\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\r\n\r\nfunction Vector2D(x, y) {\r\n    this.x = x;\r\n    this.y = y;\r\n}\r\n\r\nVector2D.prototype.plus = function(other) {\r\n    return new Vector2D(this.x + other.x,\r\n                        this.y + other.y);\r\n};\r\n\r\nVector2D.prototype.minus = function(other) {\r\n    return new Vector2D(this.x - other.x,\r\n                        this.y - other.y);\r\n};\r\n\r\nVector2D.prototype.toString = function() {\r\n    return \"[\" + this.x + \", \" + this.y + \"]\";\r\n};\r\n\r\nVector2D.prototype.compareTo = function(other) {\r\n    var result = compareNumbers(this.x, other.x);\r\n    if (result)\r\n        return result;\r\n    return compareNumbers(this.y, other.y);\r\n};\r\n\r\n","./cdjs/vector_3d.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\r\n// Copyright (C) 2015 Apple Inc. All rights reserved.\r\n// \r\n// Redistribution and use in source and binary forms, with or without\r\n// modification, are permitted provided that the following conditions are met:\r\n//  * Redistributions of source code must retain the above copyright\r\n//    notice, this list of conditions and the following disclaimer.\r\n//  * Redistributions in binary form must reproduce the above copyright\r\n//    notice, this list of conditions and the following disclaimer in the\r\n//    documentation and/or other materials provided with the distribution.\r\n//  * Neither the name of the Purdue University nor the\r\n//    names of its contributors may be used to endorse or promote products\r\n//    derived from this software without specific prior written permission.\r\n// \r\n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\r\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\r\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\r\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\r\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\r\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\r\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\r\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\r\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\r\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\r\n\r\nfunction Vector3D(x, y, z) {\r\n    this.x = x;\r\n    this.y = y;\r\n    this.z = z;\r\n}\r\n\r\nVector3D.prototype.plus = function(other) {\r\n    return new Vector3D(this.x + other.x,\r\n                        this.y + other.y,\r\n                        this.z + other.z);\r\n};\r\n\r\nVector3D.prototype.minus = function(other) {\r\n    return new Vector3D(this.x - other.x,\r\n                        this.y - other.y,\r\n                        this.z - other.z);\r\n};\r\n\r\nVector3D.prototype.dot = function(other) {\r\n    return this.x * other.x + this.y * other.y + this.z * other.z;\r\n};\r\n\r\nVector3D.prototype.squaredMagnitude = function() {\r\n    return this.dot(this);\r\n};\r\n\r\nVector3D.prototype.magnitude = function() {\r\n    return Math.sqrt(this.squaredMagnitude());\r\n};\r\n\r\nVector3D.prototype.times = function(amount) {\r\n    return new Vector3D(this.x * amount,\r\n                        this.y * amount,\r\n                        this.z * amount);\r\n};\r\n\r\nVector3D.prototype.as2D = function() {\r\n    return new Vector2D(this.x, this.y);\r\n};\r\n\r\nVector3D.prototype.toString = function() {\r\n    return \"[\" + this.x + \", \" + this.y + \", \" + this.z + \"]\";\r\n};\r\n\r\n\r\n","./cdjs/motion.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\r\n// Copyright (C) 2015 Apple Inc. All rights reserved.\r\n// \r\n// Redistribution and use in source and binary forms, with or without\r\n// modification, are permitted provided that the following conditions are met:\r\n//  * Redistributions of source code must retain the above copyright\r\n//    notice, this list of conditions and the following disclaimer.\r\n//  * Redistributions in binary form must reproduce the above copyright\r\n//    notice, this list of conditions and the following disclaimer in the\r\n//    documentation and/or other materials provided with the distribution.\r\n//  * Neither the name of the Purdue University nor the\r\n//    names of its contributors may be used to endorse or promote products\r\n//    derived from this software without specific prior written permission.\r\n// \r\n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\r\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\r\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\r\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\r\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\r\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\r\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\r\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\r\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\r\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\r\n\r\nfunction Motion(callsign, posOne, posTwo) {\r\n    this.callsign = callsign;\r\n    this.posOne = posOne;\r\n    this.posTwo = posTwo;\r\n}\r\n\r\nMotion.prototype.toString = function() {\r\n    return \"Motion(\" + this.callsign + \" from \" + this.posOne + \" to \" + this.posTwo + \")\";\r\n};\r\n\r\nMotion.prototype.delta = function() {\r\n    return this.posTwo.minus(this.posOne);\r\n};\r\n\r\nMotion.prototype.findIntersection = function(other) {\r\n    var init1 = this.posOne;\r\n    var init2 = other.posOne;\r\n    var vec1 = this.delta();\r\n    var vec2 = other.delta();\r\n    var radius = Constants.PROXIMITY_RADIUS;\r\n    \r\n    // this test is not geometrical 3-d intersection test, it takes the fact that the aircraft move\r\n    // into account ; so it is more like a 4d test\r\n    // (it assumes that both of the aircraft have a constant speed over the tested interval)\r\n    \r\n    // we thus have two points, each of them moving on its line segment at constant speed ; we are looking\r\n    // for times when the distance between these two points is smaller than r \r\n    \r\n    // vec1 is vector of aircraft 1\r\n    // vec2 is vector of aircraft 2\r\n    \r\n    // a = (V2 - V1)^T * (V2 - V1)\r\n    var a = vec2.minus(vec1).squaredMagnitude();\r\n    \r\n    if (a != 0) {\r\n        // we are first looking for instances of time when the planes are exactly r from each other\r\n        // at least one plane is moving ; if the planes are moving in parallel, they do not have constant speed\r\n\r\n        // if the planes are moving in parallel, then\r\n        //   if the faster starts behind the slower, we can have 2, 1, or 0 solutions\r\n        //   if the faster plane starts in front of the slower, we can have 0 or 1 solutions\r\n\r\n        // if the planes are not moving in parallel, then\r\n\r\n\r\n        // point P1 = I1 + vV1\r\n        // point P2 = I2 + vV2\r\n        //   - looking for v, such that dist(P1,P2) = || P1 - P2 || = r\r\n\r\n        // it follows that || P1 - P2 || = sqrt( < P1-P2, P1-P2 > )\r\n        //   0 = -r^2 + < P1 - P2, P1 - P2 >\r\n        //  from properties of dot product\r\n        //   0 = -r^2 + <I1-I2,I1-I2> + v * 2<I1-I2, V1-V2> + v^2 *<V1-V2,V1-V2>\r\n        //   so we calculate a, b, c - and solve the quadratic equation\r\n        //   0 = c + bv + av^2\r\n\r\n        // b = 2 * <I1-I2, V1-V2>\r\n\r\n        var b = 2 * init1.minus(init2).dot(vec1.minus(vec2));\r\n\r\n        // c = -r^2 + (I2 - I1)^T * (I2 - I1)\r\n        var c = -radius * radius + init2.minus(init1).squaredMagnitude();\r\n\r\n        var discr = b * b - 4 * a * c;\r\n        if (discr < 0)\r\n            return null;\r\n\r\n        var v1 = (-b - Math.sqrt(discr)) / (2 * a);\r\n        var v2 = (-b + Math.sqrt(discr)) / (2 * a);\r\n\r\n        if (v1 <= v2 && ((v1 <= 1 && 1 <= v2) ||\r\n                         (v1 <= 0 && 0 <= v2) ||\r\n                         (0 <= v1 && v2 <= 1))) {\r\n            // Pick a good \"time\" at which to report the collision.\r\n            var v;\r\n            if (v1 <= 0) {\r\n                // The collision started before this frame. Report it at the start of the frame.\r\n                v = 0;\r\n            } else {\r\n                // The collision started during this frame. Report it at that moment.\r\n                v = v1;\r\n            }\r\n            \r\n            var result1 = init1.plus(vec1.times(v));\r\n            var result2 = init2.plus(vec2.times(v));\r\n            \r\n            var result = result1.plus(result2).times(0.5);\r\n            if (result.x >= Constants.MIN_X &&\r\n                result.x <= Constants.MAX_X &&\r\n                result.y >= Constants.MIN_Y &&\r\n                result.y <= Constants.MAX_Y &&\r\n                result.z >= Constants.MIN_Z &&\r\n                result.z <= Constants.MAX_Z)\r\n                return result;\r\n        }\r\n\r\n        return null;\r\n    }\r\n    \r\n    // the planes have the same speeds and are moving in parallel (or they are not moving at all)\r\n    // they  thus have the same distance all the time ; we calculate it from the initial point\r\n    \r\n    // dist = || i2 - i1 || = sqrt(  ( i2 - i1 )^T * ( i2 - i1 ) )\r\n    \r\n    var dist = init2.minus(init1).magnitude();\r\n    if (dist <= radius)\r\n        return init1.plus(init2).times(0.5);\r\n    \r\n    return null;\r\n};\r\n\r\n","./cdjs/reduce_collision_set.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\r\n// Copyright (C) 2015 Apple Inc. All rights reserved.\r\n// \r\n// Redistribution and use in source and binary forms, with or without\r\n// modification, are permitted provided that the following conditions are met:\r\n//  * Redistributions of source code must retain the above copyright\r\n//    notice, this list of conditions and the following disclaimer.\r\n//  * Redistributions in binary form must reproduce the above copyright\r\n//    notice, this list of conditions and the following disclaimer in the\r\n//    documentation and/or other materials provided with the distribution.\r\n//  * Neither the name of the Purdue University nor the\r\n//    names of its contributors may be used to endorse or promote products\r\n//    derived from this software without specific prior written permission.\r\n// \r\n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\r\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\r\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\r\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\r\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\r\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\r\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\r\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\r\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\r\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\r\n\r\nvar drawMotionOnVoxelMap = (function() {\r\n    var voxelSize = Constants.GOOD_VOXEL_SIZE;\r\n    var horizontal = new Vector2D(voxelSize, 0);\r\n    var vertical = new Vector2D(0, voxelSize);\r\n    \r\n    function voxelHash(position) {\r\n        var xDiv = (position.x / voxelSize) | 0;\r\n        var yDiv = (position.y / voxelSize) | 0;\r\n        \r\n        var result = new Vector2D();\r\n        result.x = voxelSize * xDiv;\r\n        result.y = voxelSize * yDiv;\r\n        \r\n        if (position.x < 0)\r\n            result.x -= voxelSize;\r\n        if (position.y < 0)\r\n            result.y -= voxelSize;\r\n        \r\n        return result;\r\n    }\r\n    \r\n    return function(voxelMap, motion) {\r\n        var seen = new RedBlackTree();\r\n        \r\n        function putIntoMap(voxel) {\r\n            var array = voxelMap.get(voxel);\r\n            if (!array)\r\n                voxelMap.put(voxel, array = []);\r\n            array.push(motion);\r\n        }\r\n        \r\n        function isInVoxel(voxel) {\r\n            if (voxel.x > Constants.MAX_X ||\r\n                voxel.x < Constants.MIN_X ||\r\n                voxel.y > Constants.MAX_Y ||\r\n                voxel.y < Constants.MIN_Y)\r\n                return false;\r\n            \r\n            var init = motion.posOne;\r\n            var fin = motion.posTwo;\r\n            \r\n            var v_s = voxelSize;\r\n            var r = Constants.PROXIMITY_RADIUS / 2;\r\n            \r\n            var v_x = voxel.x;\r\n            var x0 = init.x;\r\n            var xv = fin.x - init.x;\r\n            \r\n            var v_y = voxel.y;\r\n            var y0 = init.y;\r\n            var yv = fin.y - init.y;\r\n            \r\n            var low_x, high_x;\r\n            low_x = (v_x - r - x0) / xv;\r\n            high_x = (v_x + v_s + r - x0) / xv;\r\n            \r\n            if (xv < 0) {\r\n                var tmp = low_x;\r\n                low_x = high_x;\r\n                high_x = tmp;\r\n            }\r\n            \r\n            var low_y, high_y;\r\n            low_y = (v_y - r - y0) / yv;\r\n            high_y = (v_y + v_s + r - y0) / yv;\r\n            \r\n            if (yv < 0) {\r\n                var tmp = low_y;\r\n                low_y = high_y;\r\n                high_y = tmp;\r\n            }\r\n            \r\n            if (false) {\r\n                print(\"v_x = \" + v_x + \", x0 = \" + x0 + \", xv = \" + xv + \", v_y = \" + v_y + \", y0 = \" + y0 + \", yv = \" + yv + \", low_x = \" + low_x + \", low_y = \" + low_y + \", high_x = \" + high_x + \", high_y = \" + high_y);\r\n            }\r\n            \r\n            return (((xv == 0 && v_x <= x0 + r && x0 - r <= v_x + v_s) /* no motion in x */ || \r\n                     ((low_x <= 1 && 1 <= high_x) || (low_x <= 0 && 0 <= high_x) ||\r\n                      (0 <= low_x && high_x <= 1))) && \r\n                    ((yv == 0 && v_y <= y0 + r && y0 - r <= v_y + v_s) /* no motion in y */ || \r\n                     ((low_y <= 1 && 1 <= high_y) || (low_y <= 0 && 0 <= high_y) ||\r\n                      (0 <= low_y && high_y <= 1))) && \r\n                    (xv == 0 || yv == 0 || /* no motion in x or y or both */\r\n                     (low_y <= high_x && high_x <= high_y) ||\r\n                     (low_y <= low_x && low_x <= high_y) ||\r\n                     (low_x <= low_y && high_y <= high_x)));\r\n        }\r\n        \r\n        function recurse(nextVoxel) {\r\n            if (!isInVoxel(nextVoxel, motion))\r\n                return;\r\n            if (seen.put(nextVoxel, true))\r\n                return;\r\n            \r\n            putIntoMap(nextVoxel);\r\n            \r\n            recurse(nextVoxel.minus(horizontal));\r\n            recurse(nextVoxel.plus(horizontal));\r\n            recurse(nextVoxel.minus(vertical));\r\n            recurse(nextVoxel.plus(vertical));\r\n            recurse(nextVoxel.minus(horizontal).minus(vertical));\r\n            recurse(nextVoxel.minus(horizontal).plus(vertical));\r\n            recurse(nextVoxel.plus(horizontal).minus(vertical));\r\n            recurse(nextVoxel.plus(horizontal).plus(vertical));\r\n        }\r\n        \r\n        recurse(voxelHash(motion.posOne));\r\n    };\r\n})();\r\n\r\nfunction reduceCollisionSet(motions) {\r\n    var voxelMap = new RedBlackTree();\r\n    for (var i = 0; i < motions.length; ++i)\r\n        drawMotionOnVoxelMap(voxelMap, motions[i]);\r\n        \r\n    var result = [];\r\n    voxelMap.forEach(function(key, value) {\r\n        if (value.length > 1)\r\n            result.push(value);\r\n    });\r\n    return result;\r\n}\r\n\r\n","./cdjs/simulator.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\r\n// Copyright (C) 2015 Apple Inc. All rights reserved.\r\n// \r\n// Redistribution and use in source and binary forms, with or without\r\n// modification, are permitted provided that the following conditions are met:\r\n//  * Redistributions of source code must retain the above copyright\r\n//    notice, this list of conditions and the following disclaimer.\r\n//  * Redistributions in binary form must reproduce the above copyright\r\n//    notice, this list of conditions and the following disclaimer in the\r\n//    documentation and/or other materials provided with the distribution.\r\n//  * Neither the name of the Purdue University nor the\r\n//    names of its contributors may be used to endorse or promote products\r\n//    derived from this software without specific prior written permission.\r\n// \r\n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\r\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\r\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\r\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\r\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\r\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\r\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\r\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\r\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\r\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\r\n\r\nfunction Simulator(numAircraft) {\r\n    this._aircraft = [];\r\n    for (var i = 0; i < numAircraft; ++i)\r\n        this._aircraft.push(new CallSign(\"foo\" + i));\r\n}\r\n\r\nSimulator.prototype.simulate = function(time) {\r\n    var frame = [];\r\n    for (var i = 0; i < this._aircraft.length; i += 2) {\r\n        frame.push({\r\n            callsign: this._aircraft[i],\r\n            position: new Vector3D(time, Math.cos(time) * 2 + i * 3, 10)\r\n        });\r\n        frame.push({\r\n            callsign: this._aircraft[i + 1],\r\n            position: new Vector3D(time, Math.sin(time) * 2 + i * 3, 10)\r\n        });\r\n    }\r\n    return frame;\r\n};\r\n\r\n","./cdjs/collision.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\r\n// Copyright (C) 2015 Apple Inc. All rights reserved.\r\n// \r\n// Redistribution and use in source and binary forms, with or without\r\n// modification, are permitted provided that the following conditions are met:\r\n//  * Redistributions of source code must retain the above copyright\r\n//    notice, this list of conditions and the following disclaimer.\r\n//  * Redistributions in binary form must reproduce the above copyright\r\n//    notice, this list of conditions and the following disclaimer in the\r\n//    documentation and/or other materials provided with the distribution.\r\n//  * Neither the name of the Purdue University nor the\r\n//    names of its contributors may be used to endorse or promote products\r\n//    derived from this software without specific prior written permission.\r\n// \r\n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\r\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\r\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\r\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\r\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\r\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\r\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\r\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\r\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\r\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\r\n\r\nfunction Collision(aircraft, position) {\r\n    this.aircraft = aircraft;\r\n    this.position = position;\r\n}\r\n\r\nCollision.prototype.toString = function() {\r\n    return \"Collision(\" + this.aircraft + \" at \" + this.position + \")\";\r\n};\r\n\r\n","./cdjs/collision_detector.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\r\n// Copyright (C) 2015 Apple Inc. All rights reserved.\r\n// \r\n// Redistribution and use in source and binary forms, with or without\r\n// modification, are permitted provided that the following conditions are met:\r\n//  * Redistributions of source code must retain the above copyright\r\n//    notice, this list of conditions and the following disclaimer.\r\n//  * Redistributions in binary form must reproduce the above copyright\r\n//    notice, this list of conditions and the following disclaimer in the\r\n//    documentation and/or other materials provided with the distribution.\r\n//  * Neither the name of the Purdue University nor the\r\n//    names of its contributors may be used to endorse or promote products\r\n//    derived from this software without specific prior written permission.\r\n// \r\n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\r\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\r\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\r\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\r\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\r\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\r\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\r\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\r\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\r\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\r\n\r\nfunction CollisionDetector() {\r\n    this._state = new RedBlackTree();\r\n}\r\n\r\nCollisionDetector.prototype.handleNewFrame = function(frame) {\r\n    var motions = [];\r\n    var seen = new RedBlackTree();\r\n    \r\n    for (var i = 0; i < frame.length; ++i) {\r\n        var aircraft = frame[i];\r\n        \r\n        var oldPosition = this._state.put(aircraft.callsign, aircraft.position);\r\n        var newPosition = aircraft.position;\r\n        seen.put(aircraft.callsign, true);\r\n        \r\n        if (!oldPosition) {\r\n            // Treat newly introduced aircraft as if they were stationary.\r\n            oldPosition = newPosition;\r\n        }\r\n        \r\n        motions.push(new Motion(aircraft.callsign, oldPosition, newPosition));\r\n    }\r\n    \r\n    // Remove aircraft that are no longer present.\r\n    var toRemove = [];\r\n    this._state.forEach(function(callsign, position) {\r\n        if (!seen.get(callsign))\r\n            toRemove.push(callsign);\r\n    });\r\n    for (var i = 0; i < toRemove.length; ++i)\r\n        this._state.remove(toRemove[i]);\r\n    \r\n    var allReduced = reduceCollisionSet(motions);\r\n    var collisions = [];\r\n    for (var reductionIndex = 0; reductionIndex < allReduced.length; ++reductionIndex) {\r\n        var reduced = allReduced[reductionIndex];\r\n        for (var i = 0; i < reduced.length; ++i) {\r\n            var motion1 = reduced[i];\r\n            for (var j = i + 1; j < reduced.length; ++j) {\r\n                var motion2 = reduced[j];\r\n                var collision = motion1.findIntersection(motion2);\r\n                if (collision)\r\n                    collisions.push(new Collision([motion1.callsign, motion2.callsign], collision));\r\n            }\r\n        }\r\n    }\r\n    \r\n    return collisions;\r\n};\r\n","./cdjs/benchmark.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\r\n// Copyright (C) 2015-2016 Apple Inc. All rights reserved.\r\n// \r\n// Redistribution and use in source and binary forms, with or without\r\n// modification, are permitted provided that the following conditions are met:\r\n//  * Redistributions of source code must retain the above copyright\r\n//    notice, this list of conditions and the following disclaimer.\r\n//  * Redistributions in binary form must reproduce the above copyright\r\n//    notice, this list of conditions and the following disclaimer in the\r\n//    documentation and/or other materials provided with the distribution.\r\n//  * Neither the name of the Purdue University nor the\r\n//    names of its contributors may be used to endorse or promote products\r\n//    derived from this software without specific prior written permission.\r\n// \r\n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\r\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\r\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\r\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\r\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\r\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\r\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\r\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\r\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\r\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\r\n\r\nfunction benchmarkImpl(configuration) {\r\n    var verbosity = configuration.verbosity;\r\n    var numAircraft = configuration.numAircraft;\r\n    var numFrames = configuration.numFrames;\r\n    var expectedCollisions = configuration.expectedCollisions;\r\n    var exclude = configuration.exclude;\r\n\r\n    var simulator = new Simulator(numAircraft);\r\n    var detector = new CollisionDetector();\r\n    var lastTime = currentTime();\r\n    var results = [];\r\n    for (var i = 0; i < numFrames; ++i) {\r\n        var time = i / 10;\r\n        \r\n        var collisions = detector.handleNewFrame(simulator.simulate(time));\r\n        \r\n        var before = lastTime;\r\n        var after = currentTime();\r\n        lastTime = after;\r\n        var result = {\r\n            time: after - before,\r\n            numCollisions: collisions.length\r\n        };\r\n        if (verbosity >= 2)\r\n            print(\"CDjs: \" + result.time);\r\n        if (verbosity >= 3)\r\n            result.collisions = collisions;\r\n        results.push(result);\r\n    }\r\n    \r\n    results.splice(0, exclude);\r\n\r\n    if (verbosity >= 1) {\r\n        for (var i = 0; i < results.length; ++i) {\r\n            var string = \"Frame \" + i + \": \" + results[i].time + \" ms.\";\r\n            if (results[i].numCollisions)\r\n                string += \" (\" + results[i].numCollisions + \" collisions.)\";\r\n            print(string);\r\n            if (verbosity >= 2 && results[i].collisions.length)\r\n                print(\"    Collisions: \" + results[i].collisions);\r\n        }\r\n    }\r\n\r\n    // Check results.\r\n    var actualCollisions = 0;\r\n    for (var i = 0; i < results.length; ++i)\r\n        actualCollisions += results[i].numCollisions;\r\n    if (actualCollisions != expectedCollisions) {\r\n        throw new Error(\"Bad number of collisions: \" + actualCollisions + \" (expected \" + expectedCollisions + \")\");\r\n    }\r\n}\r\n\r\nfunction benchmark() {\r\n    return benchmarkImpl({\r\n        verbosity: 0,\r\n        numAircraft: 1000,\r\n        numFrames: 18,\r\n        expectedCollisions: 1336,\r\n        exclude: 0\r\n    });\r\n}\r\n\r\nclass Benchmark {\r\n    runIteration() {\r\n        benchmark();\r\n    }\r\n}\r\n"};
var readFile = function (name) {
    const normalized = String(name).replaceAll("\\", "/");
    if (!Object.prototype.hasOwnProperty.call(__jetstreamResources, normalized))
        throw new Error("JetStream resource not embedded: " + normalized);
    return __jetstreamResources[normalized];
};

"use strict";

/*
 * Copyright (C) 2018 Apple Inc. All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY APPLE INC. AND ITS CONTRIBUTORS ``AS IS''
 * AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO,
 * THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
 * PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL APPLE INC. OR ITS CONTRIBUTORS
 * BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
 * CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
 * SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
 * CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
 * ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF
 * THE POSSIBILITY OF SUCH DAMAGE.
*/

const preloadResources = !isInBrowser;
const measureTotalTimeAsSubtest = false; // Once we move to preloading all resources, it would be good to turn this on.

if (typeof RAMification === "undefined")
    var RAMification = false;

if (typeof testIterationCount === "undefined")
    var testIterationCount = undefined;

// Used for the promise representing the current benchmark run.
var currentResolve = null;
var currentReject = null;

const defaultIterationCount = 120;
const defaultWorstCaseCount = 4;

function assert(b, m = "") {
    if (!b)
        throw new Error("Bad assertion: " + m);
}

function firstID(benchmark) {
    return `results-cell-${benchmark.name}-first`;
}

function worst4ID(benchmark) {
    return `results-cell-${benchmark.name}-worst4`;
}

function avgID(benchmark) {
    return `results-cell-${benchmark.name}-avg`;
}

function scoreID(benchmark) {
    return `results-cell-${benchmark.name}-score`;
}

function mean(values) {
    assert(values instanceof Array);
    let sum = 0;
    for (let x of values)
        sum += x;
    return sum / values.length;
}

function geomean(values) {
    assert(values instanceof Array);
    let product = 1;
    for (let x of values)
        product *= x;
    return product ** (1 / values.length);
}

function toScore(timeValue) {
    return 5000 / timeValue;
}

function toTimeValue(score) {
    return 5000 / score;
}

function updateUI() {
    return new Promise((resolve) => {
        if (isInBrowser)
            requestAnimationFrame(() => setTimeout(resolve, 0));
        else
            resolve();
    });
}

function uiFriendlyNumber(num) {
    if (Number.isInteger(num))
        return num;
    return num.toFixed(3);
}

function uiFriendlyDuration(time)
{
    let minutes = time.getMinutes();
    let seconds = time.getSeconds();
    let milliSeconds = time.getMilliseconds();
    let result = "" + minutes + ":";

    result = result + (seconds < 10 ? "0" : "") + seconds + ".";
    result = result + (milliSeconds < 10 ? "00" : (milliSeconds < 100 ? "0" : "")) + milliSeconds;

    return result;
}

const fileLoader = (function() {
    class Loader {
        constructor() {
            this.requests = new Map;
        }

        async _loadInternal(url) {
            if (!isInBrowser)
                return Promise.resolve(readFile(url));

            let fetchResponse = await fetch(new Request(url));
            if (url.indexOf(".js") !== -1)
                return await fetchResponse.text();
            else if (url.indexOf(".wasm") !== -1)
                return await fetchResponse.arrayBuffer();

            throw new Error("should not be reached!");
        }

        async load(url) {
            if (this.requests.has(url))
                return (await this.requests.get(url));

            let promise = this._loadInternal(url);
            this.requests.set(url, promise);
            return (await promise);
        }
    }
    return new Loader;
})();

class Driver {
    constructor() {
        this.benchmarks = [];
    }

    addPlan(plan, BenchmarkClass = DefaultBenchmark) {
        this.benchmarks.push(new BenchmarkClass(plan));
    }

    async start() {
        let statusElement = false;
        let summaryElement = false;
        if (isInBrowser) {
            statusElement = document.getElementById("status");
            summaryElement = document.getElementById("result-summary");
            statusElement.innerHTML = `<label>Running...</label>`;
        } else {
            console.log("Starting JetStream2");
        }

        await updateUI();

        let __jetstreamSuiteStart = Date.now();
        for (let benchmark of this.benchmarks) {
            benchmark.updateUIBeforeRun();

            await updateUI();

            try {

                await benchmark.run();
            } catch(e) {
                JetStream.reportError(benchmark);
                throw e;
            }

            benchmark.updateUIAfterRun();
        }

        let totalTime = Date.now() - __jetstreamSuiteStart;
        if (measureTotalTimeAsSubtest) {
            if (isInBrowser)
                document.getElementById("benchmark-total-time-score").innerHTML = uiFriendlyNumber(totalTime);
            else
                console.log("Total time:", uiFriendlyNumber(totalTime));
            allScores.push(totalTime);
        }

        let allScores = [];
        for (let benchmark of this.benchmarks)
            allScores.push(benchmark.score);

        if (isInBrowser) {
            summaryElement.classList.add('done');
            summaryElement.innerHTML = "<div class=\"score\">" + uiFriendlyNumber(geomean(allScores)) + "</div><label>Score</label>";
            statusElement.innerHTML = '';
        } else
            console.log("\nTotal Score: ", uiFriendlyNumber(geomean(allScores)), "\n");

        this.reportScoreToRunBenchmarkRunner();
    }

    runCode(string)
    {
        if (!isInBrowser) {
            let top = { currentResolve, currentReject };
            new Function("top", string.join("\n"))(top);
            return globalThis;
        }

        var magic = document.getElementById("magic");
        magic.contentDocument.body.textContent = "";
        magic.contentDocument.body.innerHTML = "<iframe id=\"magicframe\" frameborder=\"0\">";

        var magicFrame = magic.contentDocument.getElementById("magicframe");
        magicFrame.contentDocument.open();
        magicFrame.contentDocument.write("<!DOCTYPE html><head><title>benchmark payload</title></head><body>\n" + string + "</body></html>");

        return magicFrame;
    }

    prepareToRun()
    {
        this.benchmarks.sort((a, b) => a.plan.name.toLowerCase() < b.plan.name.toLowerCase() ? 1 : -1);

        let text = "";
        let newBenchmarks = [];
        for (let benchmark of this.benchmarks) {
            let id = JSON.stringify(benchmark.constructor.scoreDescription());
            let description = JSON.parse(id);

            newBenchmarks.push(benchmark);
            let scoreIds = benchmark.scoreIdentifiers()
            let overallScoreId = scoreIds.pop();

            if (isInBrowser) {
                text +=
                    `<div class="benchmark" id="benchmark-${benchmark.name}">
                    <h3 class="benchmark-name"><a href="in-depth.html#${benchmark.name}">${benchmark.name}</a></h3>
                    <h4 class="score" id="${overallScoreId}">___</h4><p>`;
                for (let i = 0; i < scoreIds.length; i++) {
                    let id = scoreIds[i];
                    let label = description[i];
                    text += `<span class="result"><span id="${id}">___</span><label>${label}</label></span>`
                }
                text += `</p></div>`;
            }
        }

        if (!isInBrowser)
            return;

        for (let f = 0; f < 5; f++)
            text += `<div class="benchmark fill"></div>`;

        let timestamp = Date.now();
        document.getElementById('jetstreams').style.backgroundImage = `url('jetstreams.svg?${timestamp}')`;
        let resultsTable = document.getElementById("results");
        resultsTable.innerHTML = text;

        document.getElementById("magic").textContent = "";
        document.addEventListener('keypress', function (e) {
            if (e.which === 13)
                JetStream.start();
        });
    }

    reportError(benchmark)
    {
        for (let id of benchmark.scoreIdentifiers())
            document.getElementById(id).innerHTML = "error";
    }

    async initialize() {
        await this.fetchResources();
        this.prepareToRun();
        if (isInBrowser && window.location.search == '?report=true') {
            setTimeout(() => this.start(), 4000);
        }
    }

    async fetchResources() {
        for (let benchmark of this.benchmarks)
            await benchmark.fetchResources();

        if (!isInBrowser)
            return;

        let statusElement = document.getElementById("status");
        statusElement.classList.remove('loading');
        statusElement.innerHTML = `<a href="javascript:JetStream.start()" class="button">Start Test</a>`;
        statusElement.onclick = () => {
            statusElement.onclick = null;
            JetStream.start();
            return false;
        }
    }

    async reportScoreToRunBenchmarkRunner()
    {
        if (!isInBrowser)
            return;

        if (window.location.search !== '?report=true')
            return;

        let results = {};
        for (let benchmark of this.benchmarks) {
            const subResults = {}
            const subTimes = benchmark.subTimes();
            for (const name in subTimes) {
                subResults[name] = {"metrics": {"Time": {"current": [toTimeValue(subTimes[name])]}}};
            }
            results[benchmark.name] = {
                "metrics" : {
                    "Score" : {"current" : [benchmark.score]},
                    "Time": ["Geometric"],
                },
                "tests": subResults,
            };;
        }

        results = {"JetStream2.0": {"metrics" : {"Score" : ["Geometric"]}, "tests" : results}};

        const content = JSON.stringify(results);
        await fetch("/report", {
            method: "POST",
            heeaders: {
                "Content-Type": "application/json",
                "Content-Length": content.length,
                "Connection": "close",
            },
            body: content,
        });
    }
};

class Benchmark {
    constructor(plan)
    {
        this.plan = plan;
        this.iterations = testIterationCount || plan.iterations || defaultIterationCount;
        this.isAsync = !!plan.isAsync;

        this.scripts = null;

        this._resourcesPromise = Promise.resolve();
        this.scripts = this.plan.files.map((file) => readFile(file));
    }

    get name() { return this.plan.name; }

    get runnerCode() {
        return `
            let __benchmark = new Benchmark(${this.iterations});
            let results = [];
            for (let i = 0; i < ${this.iterations}; i++) {
                if (__benchmark.prepareForNextIteration)
                    __benchmark.prepareForNextIteration();

                let __jetstreamIterationStart = Date.now();
                __benchmark.runIteration();
                let __jetstreamIterationEnd = Date.now();

                results.push(Math.max(1, __jetstreamIterationEnd - __jetstreamIterationStart));
            }
            if (__benchmark.validate)
                __benchmark.validate();
            top.currentResolve(results);`;
    }

    processResults() {
        throw new Error("Subclasses need to implement this");
    }

    get score() {
        throw new Error("Subclasses need to implement this");
    }

    get prerunCode() { return null; }

    async run() {
        let code;
        if (isInBrowser)
            code = "";
        else
            code = [];

        let addScript = (text) => {
            if (isInBrowser)
                code += `<script>${text}</script>`;
            else
                code.push(text);
        };

        let addScriptWithURL = (url) => {
            if (isInBrowser)
                code += `<script src="${url}"></script>`;
            else
                assert(false, "Should not reach here in CLI");
        };

        addScript(`var performance = globalThis.performance = {now: Date.now.bind(Date)};`);

        if (!!this.plan.deterministicRandom) {
            addScript(`
                Math.random = (function() {
                    var seed = 49734321;
                    return function() {
                        // Robert Jenkins' 32 bit integer hash function.
                        seed = ((seed + 0x7ed55d16) + (seed << 12))  & 0xffffffff;
                        seed = ((seed ^ 0xc761c23c) ^ (seed >>> 19)) & 0xffffffff;
                        seed = ((seed + 0x165667b1) + (seed << 5))   & 0xffffffff;
                        seed = ((seed + 0xd3a2646c) ^ (seed << 9))   & 0xffffffff;
                        seed = ((seed + 0xfd7046c5) + (seed << 3))   & 0xffffffff;
                        seed = ((seed ^ 0xb55a4f09) ^ (seed >>> 16)) & 0xffffffff;
                        return (seed & 0xfffffff) / 0x10000000;
                    };
                })();
            `);

        }

        if (this.plan.preload) {
            let str = "";
            for (let [variableName, blobUrl] of this.preloads)
                str += `const ${variableName} = "${blobUrl}";\n`;
            addScript(str);
        }

        let prerunCode = this.prerunCode;
        if (prerunCode)
            addScript(prerunCode);

        if (preloadResources) {
            assert(this.scripts && this.scripts.length === this.plan.files.length);

            for (let text of this.scripts)
                addScript(text);
        } else {
            for (let file of this.plan.files)
                addScriptWithURL(file);
        }

        let promise = new Promise((resolve, reject) => {
            currentResolve = resolve;
            currentReject = reject;
        });

        if (isInBrowser) {
            code = `
                <script> window.onerror = top.currentReject; </script>
                ${code}
            `;
        }
        addScript("(() => {\n" + this.runnerCode + "\n})();");

        this.startTime = new Date();

        if (RAMification)
            resetMemoryPeak();

        let magicFrame;
        try {
            magicFrame = JetStream.runCode(code);
        } catch(e) {
            console.log("Error in runCode: ", e);
            throw e;
        }
        let results = await promise;

        this.endTime = new Date();

        if (RAMification) {
            let memoryFootprint = MemoryFootprint();
            this.currentFootprint = memoryFootprint.current;
            this.peakFootprint = memoryFootprint.peak;
        }

        this.processResults(results);
        if (isInBrowser)
            magicFrame.contentDocument.close();
    }

    fetchResources() {
        if (this._resourcesPromise)
            return this._resourcesPromise;

        let filePromises = preloadResources ? this.plan.files.map((file) => fileLoader.load(file)) : [];
        let preloads = [];
        let preloadVariableNames = [];

        if (isInBrowser && this.plan.preload) {
            for (let prop of Object.getOwnPropertyNames(this.plan.preload)) {
                preloadVariableNames.push(prop);
                preloads.push(this.plan.preload[prop]);
            }
        }

        preloads = preloads.map((file) => fileLoader.load(file));

        let p1 = Promise.all(filePromises).then((texts) => {
            if (!preloadResources)
                return;
            this.scripts = [];
            assert(texts.length === this.plan.files.length);
            for (let text of texts)
                this.scripts.push(text);
        });

        let p2 = Promise.all(preloads).then((data) => {
            this.preloads = [];
            this.blobs = [];
            for (let i = 0; i < data.length; ++i) {
                let item = data[i];

                let blob;
                if (typeof item === "string") {
                    blob = new Blob([item], {type : 'application/javascript'});
                } else if (item instanceof ArrayBuffer) {
                    blob = new Blob([item], {type : 'application/octet-stream'});
                } else
                    throw new Error("Unexpected item!");

                this.blobs.push(blob);
                this.preloads.push([preloadVariableNames[i], URL.createObjectURL(blob)]);
            }
        });

        this._resourcesPromise = Promise.all([p1, p2]);
        return this._resourcesPromise;
    }

    static scoreDescription() { throw new Error("Must be implemented by subclasses."); }
    scoreIdentifiers() { throw new Error("Must be implemented by subclasses"); }

    updateUIBeforeRun() {
        if (!isInBrowser) {
            console.log(`Running ${this.name}:`);
            return;
        }

        let containerUI = document.getElementById("results");
        let resultsBenchmarkUI = document.getElementById(`benchmark-${this.name}`);
        containerUI.insertBefore(resultsBenchmarkUI, containerUI.firstChild);
        resultsBenchmarkUI.classList.add("benchmark-running");

        for (let id of this.scoreIdentifiers())
            document.getElementById(id).innerHTML = "...";
    }

    updateUIAfterRun() {
        if (!isInBrowser)
            return;

        let benchmarkResultsUI = document.getElementById(`benchmark-${this.name}`);
        benchmarkResultsUI.classList.remove("benchmark-running");
        benchmarkResultsUI.classList.add("benchmark-done");

    }
};

class DefaultBenchmark extends Benchmark {
    constructor(...args) {
        super(...args);

        this.worstCaseCount = this.plan.worstCaseCount || defaultWorstCaseCount;
        this.firstIteration = null;
        this.worst4 = null;
        this.average = null;
    }

    processResults(results) {
        function copyArray(a) {
            let result = [];
            for (let x of a)
                result.push(x);
            return result;
        }
        results = copyArray(results);

        this.firstIteration = toScore(results[0]);

        results = results.slice(1);
        results.sort((a, b) => a < b ? 1 : -1);
        for (let i = 0; i + 1 < results.length; ++i)
            assert(results[i] >= results[i + 1]);

        let worstCase = [];
        for (let i = 0; i < this.worstCaseCount; ++i)
            worstCase.push(results[i]);
        this.worst4 = toScore(mean(worstCase));
        this.average = toScore(mean(results));
    }

    get score() {
        return geomean([this.firstIteration, this.worst4, this.average]);
    }

    subTimes() {
        return {
            "First": this.firstIteration,
            "Worst": this.worst4,
            "Average": this.average,
        };
    }

    static scoreDescription() {
        return ["First", "Worst", "Average", "Score"];
    }

    scoreIdentifiers() {
        return [firstID(this), worst4ID(this), avgID(this), scoreID(this)];
    }

    updateUIAfterRun() {
        super.updateUIAfterRun();

        if (isInBrowser) {
            document.getElementById(firstID(this)).innerHTML = uiFriendlyNumber(this.firstIteration);
            document.getElementById(worst4ID(this)).innerHTML = uiFriendlyNumber(this.worst4);
            document.getElementById(avgID(this)).innerHTML = uiFriendlyNumber(this.average);
            document.getElementById(scoreID(this)).innerHTML = uiFriendlyNumber(this.score);
            return;
        }

        print("    Startup:", uiFriendlyNumber(this.firstIteration));
        print("    Worst Case:", uiFriendlyNumber(this.worst4));
        print("    Average:", uiFriendlyNumber(this.average));
        print("    Score:", uiFriendlyNumber(this.score));
        if (RAMification) {
            print("    Current Footprint:", uiFriendlyNumber(this.currentFootprint));
            print("    Peak Footprint:", uiFriendlyNumber(this.peakFootprint));
        }
        print("    Wall time:", uiFriendlyDuration(new Date(this.endTime - this.startTime)));
    }
}

class AsyncBenchmark extends DefaultBenchmark {
    get runnerCode() {
        return `
        async function doRun() {
            let __benchmark = new Benchmark();
            let results = [];
            for (let i = 0; i < ${this.iterations}; i++) {
                let start = Date.now();
                await __benchmark.runIteration();
                let end = Date.now();
                results.push(Math.max(1, end - start));
            }
            if (__benchmark.validate)
                __benchmark.validate();
            top.currentResolve(results);
        }
        doRun();`
    }
};

class WSLBenchmark extends Benchmark {
    constructor(...args) {
        super(...args);

        this.stdlib = null;
        this.mainRun = null;
    }

    processResults(results) {
        this.stdlib = toScore(results[0]);
        this.mainRun = toScore(results[1]);
    }

    get score() {
        return geomean([this.stdlib, this.mainRun]);
    }

    get runnerCode() {
        return `
            let benchmark = new Benchmark();
            let results = [];
            {
                let start = Date.now();
                benchmark.buildStdlib();
                results.push(Date.now() - start);
            }

            {
                let start = Date.now();
                benchmark.run();
                results.push(Date.now() - start);
            }

            top.currentResolve(results);
            `;
    }

    subTimes() {
        return {
            "Stdlib": this.stdlib,
            "MainRun": this.mainRun,
        };
    }

    static scoreDescription() {
        return ["Stdlib", "MainRun", "Score"];
    }

    scoreIdentifiers() {
        return ["wsl-stdlib-score", "wsl-tests-score", "wsl-score-score"];
    }

    updateUIAfterRun() {
        super.updateUIAfterRun();

        if (isInBrowser) {
            document.getElementById("wsl-stdlib-score").innerHTML = uiFriendlyNumber(this.stdlib);
            document.getElementById("wsl-tests-score").innerHTML = uiFriendlyNumber(this.mainRun);
            document.getElementById("wsl-score-score").innerHTML = uiFriendlyNumber(this.score);
            return;
        }

        print("    Stdlib:", uiFriendlyNumber(this.stdlib));
        print("    Tests:", uiFriendlyNumber(this.mainRun));
        print("    Score:", uiFriendlyNumber(this.score));
        if (RAMification) {
            print("    Current Footprint:", uiFriendlyNumber(this.currentFootprint));
            print("    Peak Footprint:", uiFriendlyNumber(this.peakFootprint));
        }
        print("    Wall time:", uiFriendlyDuration(new Date(this.endTime - this.startTime)));
    }
};

class WasmBenchmark extends Benchmark {
    constructor(...args) {
        super(...args);

        this.startupTime = null;
        this.runTime = null;
    }

    processResults(results) {
        this.startupTime = toScore(results[0]);
        this.runTime = toScore(results[1]);
    }

    get score() {
        return geomean([this.startupTime, this.runTime]);
    }

    get wasmPath() {
        return this.plan.wasmPath;
    }

    get prerunCode() {
        let str = `
            let verbose = false;

            let compileTime = null;
            let runTime = null;

            let globalObject = this;

            globalObject.benchmarkTime = Date.now.bind(Date);

            globalObject.reportCompileTime = (t) => {
                if (compileTime !== null)
                    throw new Error("called report compile time twice");
                compileTime = t;
            };

            globalObject.reportRunTime = (t) => {
                if (runTime !== null)
                    throw new Error("called report run time twice")
                runTime = t;
                top.currentResolve([compileTime, runTime]);
            };

            abort = quit = function() {
                if (verbose)
                    console.log('Intercepted quit/abort');
            };

            oldPrint = globalObject.print;
            globalObject.print = globalObject.printErr = (...args) => {
                if (verbose)
                    console.log('Intercepted print: ', ...args);
            };

            let Module = {
                preRun: [],
                postRun: [],
                print: function() { },
                printErr: function() { },
                setStatus: function(text) {
                },
                totalDependencies: 0,
                monitorRunDependencies: function(left) {
                    this.totalDependencies = Math.max(this.totalDependencies, left);
                    Module.setStatus(left ? 'Preparing... (' + (this.totalDependencies-left) + '/' + this.totalDependencies + ')' : 'All downloads complete.');
                }
            };
            globalObject.Module = Module;
            `;
        return str;
    }

    get runnerCode() {
        let str = "";
        if (isInBrowser) {
            str += `
                var xhr = new XMLHttpRequest();
                xhr.open('GET', wasmBlobURL, true);
                xhr.responseType = 'arraybuffer';
                xhr.onload = function() {
                    Module.wasmBinary = xhr.response;
                    doRun();
                };
                xhr.send(null);
            `;
        } else {
            str += `
            Module.wasmBinary = read("${this.wasmPath}", "binary");
            globalObject.read = (...args) => {
                console.log("should not be inside read: ", ...args);
                throw new Error;
            };

            Module.setStatus = null;
            Module.monitorRunDependencies = null;

            Promise.resolve(42).then(() => {
                try {
                    doRun();
                } catch(e) {
                    console.log("error running wasm:", e);
                    throw e;
                }
            })
            `;
        }
        return str;
    }

    subTimes() {
        return {
            "Startup": this.startupTime,
            "Runtime": this.runTime,
        };
    }

    static scoreDescription() {
        return ["Startup", "Runtime", "Score"];
    }

    get startupID() {
        return `wasm-startup-id${this.name}`;
    }
    get runID() {
        return `wasm-run-id${this.name}`;
    }
    get scoreID() {
        return `wasm-score-id${this.name}`;
    }

    scoreIdentifiers() {
        return [this.startupID, this.runID, this.scoreID];
    }

    updateUIAfterRun() {
        super.updateUIAfterRun();

        if (isInBrowser) {
            document.getElementById(this.startupID).innerHTML = uiFriendlyNumber(this.startupTime);
            document.getElementById(this.runID).innerHTML = uiFriendlyNumber(this.runTime);
            document.getElementById(this.scoreID).innerHTML = uiFriendlyNumber(this.score);
            return;
        }
        print("    Startup:", uiFriendlyNumber(this.startupTime));
        print("    Run time:", uiFriendlyNumber(this.runTime));
        if (RAMification) {
            print("    Current Footprint:", uiFriendlyNumber(this.currentFootprint));
            print("    Peak Footprint:", uiFriendlyNumber(this.peakFootprint));
        }
        print("    Score:", uiFriendlyNumber(this.score));
    }
};

const ARESGroup = Symbol.for("ARES");
const CDJSGroup = Symbol.for("CDJS");
const CodeLoadGroup = Symbol.for("CodeLoad");
const LuaJSFightGroup = Symbol.for("LuaJSFight");
const OctaneGroup = Symbol.for("Octane");
const RexBenchGroup = Symbol.for("RexBench");
const SeaMonsterGroup = Symbol.for("SeaMonster");
const SimpleGroup = Symbol.for("Simple");
const SunSpiderGroup = Symbol.for("SunSpider");
const WasmGroup = Symbol.for("Wasm");
const WorkerTestsGroup = Symbol.for("WorkerTests");
const WSLGroup = Symbol.for("WSL");
const WTBGroup = Symbol.for("WTB");


let testPlans = [
    // ARES
    {
        name: "Air",
        files: [
            "./ARES-6/Air/symbols.js"
            , "./ARES-6/Air/tmp_base.js"
            , "./ARES-6/Air/arg.js"
            , "./ARES-6/Air/basic_block.js"
            , "./ARES-6/Air/code.js"
            , "./ARES-6/Air/frequented_block.js"
            , "./ARES-6/Air/inst.js"
            , "./ARES-6/Air/opcode.js"
            , "./ARES-6/Air/reg.js"
            , "./ARES-6/Air/stack_slot.js"
            , "./ARES-6/Air/tmp.js"
            , "./ARES-6/Air/util.js"
            , "./ARES-6/Air/custom.js"
            , "./ARES-6/Air/liveness.js"
            , "./ARES-6/Air/insertion_set.js"
            , "./ARES-6/Air/allocate_stack.js"
            , "./ARES-6/Air/payload-gbemu-executeIteration.js"
            , "./ARES-6/Air/payload-imaging-gaussian-blur-gaussianBlur.js"
            , "./ARES-6/Air/payload-airjs-ACLj8C.js"
            , "./ARES-6/Air/payload-typescript-scanIdentifier.js"
            , "./ARES-6/Air/benchmark.js"
        ],
        testGroup: ARESGroup
    },
    {
        name: "Basic",
        files: [
            "./ARES-6/Basic/ast.js"
            , "./ARES-6/Basic/basic.js"
            , "./ARES-6/Basic/caseless_map.js"
            , "./ARES-6/Basic/lexer.js"
            , "./ARES-6/Basic/number.js"
            , "./ARES-6/Basic/parser.js"
            , "./ARES-6/Basic/random.js"
            , "./ARES-6/Basic/state.js"
            , "./ARES-6/Basic/util.js"
            , "./ARES-6/Basic/benchmark.js"
        ],
        testGroup: ARESGroup
    },
    {
        name: "ML",
        files: [
            "./ARES-6/ml/index.js"
            , "./ARES-6/ml/benchmark.js"
        ],
        iterations: 60,
        testGroup: ARESGroup
    },
    {
        name: "Babylon",
        files: [
            "./ARES-6/Babylon/index.js"
            , "./ARES-6/Babylon/benchmark.js"
        ],
        preload: {
            airBlob: "./ARES-6/Babylon/air-blob.js",
            basicBlob: "./ARES-6/Babylon/basic-blob.js",
            inspectorBlob: "./ARES-6/Babylon/inspector-blob.js",
            babylonBlob: "./ARES-6/Babylon/babylon-blob.js"
        },
        testGroup: ARESGroup
    },
    // CDJS
    {
        name: "cdjs",
        files: [
            "./cdjs/constants.js"
            , "./cdjs/util.js"
            , "./cdjs/red_black_tree.js"
            , "./cdjs/call_sign.js"
            , "./cdjs/vector_2d.js"
            , "./cdjs/vector_3d.js"
            , "./cdjs/motion.js"
            , "./cdjs/reduce_collision_set.js"
            , "./cdjs/simulator.js"
            , "./cdjs/collision.js"
            , "./cdjs/collision_detector.js"
            , "./cdjs/benchmark.js"
        ],
        iterations: 60,
        worstCaseCount: 3,
        testGroup: CDJSGroup
    },
    // CodeLoad
    {
        name: "first-inspector-code-load",
        files: [
            "./code-load/code-first-load.js"
        ],
        preload: {
            inspectorPayloadBlob: "./code-load/inspector-payload-minified.js"
        },
        testGroup: CodeLoadGroup
    },
    {
        name: "multi-inspector-code-load",
        files: [
            "./code-load/code-multi-load.js"
        ],
        preload: {
            inspectorPayloadBlob: "./code-load/inspector-payload-minified.js"
        },
        testGroup: CodeLoadGroup
    },
    // Octane
    {
        name: "Box2D",
        files: [
            "./Octane/box2d.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "octane-code-load",
        files: [
            "./Octane/code-first-load.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "crypto",
        files: [
            "./Octane/crypto.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "delta-blue",
        files: [
            "./Octane/deltablue.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "earley-boyer",
        files: [
            "./Octane/earley-boyer.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "gbemu",
        files: [
            "./Octane/gbemu-part1.js"
            , "./Octane/gbemu-part2.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "mandreel",
        files: [
            "./Octane/mandreel.js"
        ],
        iterations: 80,
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "navier-stokes",
        files: [
            "./Octane/navier-stokes.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "pdfjs",
        files: [
            "./Octane/pdfjs.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "raytrace",
        files: [
            "./Octane/raytrace.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "regexp",
        files: [
            "./Octane/regexp.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "richards",
        files: [
            "./Octane/richards.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "splay",
        files: [
            "./Octane/splay.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "typescript",
        files: [
            "./Octane/typescript-compiler.js"
            , "./Octane/typescript-input.js"
            , "./Octane/typescript.js"
        ],
        iterations: 15,
        worstCaseCount: 2,
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "octane-zlib",
        files: [
            "./Octane/zlib-data.js"
            , "./Octane/zlib.js"
        ],
        iterations: 15,
        worstCaseCount: 2,
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    // RexBench
    {
        name: "FlightPlanner",
        files: [
            "./RexBench/FlightPlanner/airways.js"
            , "./RexBench/FlightPlanner/waypoints.js"
            , "./RexBench/FlightPlanner/flight_planner.js"
            , "./RexBench/FlightPlanner/expectations.js"
            , "./RexBench/FlightPlanner/benchmark.js"
        ],
        testGroup: RexBenchGroup
    },
    {
        name: "OfflineAssembler",
        files: [
            "./RexBench/OfflineAssembler/registers.js"
            , "./RexBench/OfflineAssembler/instructions.js"
            , "./RexBench/OfflineAssembler/ast.js"
            , "./RexBench/OfflineAssembler/parser.js"
            , "./RexBench/OfflineAssembler/file.js"
            , "./RexBench/OfflineAssembler/LowLevelInterpreter.js"
            , "./RexBench/OfflineAssembler/LowLevelInterpreter32_64.js"
            , "./RexBench/OfflineAssembler/LowLevelInterpreter64.js"
            , "./RexBench/OfflineAssembler/InitBytecodes.js"
            , "./RexBench/OfflineAssembler/expected.js"
            , "./RexBench/OfflineAssembler/benchmark.js"
        ],
        iterations: 80,
        testGroup: RexBenchGroup
    },
    {
        name: "UniPoker",
        files: [
            "./RexBench/UniPoker/poker.js"
            , "./RexBench/UniPoker/expected.js"
            , "./RexBench/UniPoker/benchmark.js"
        ],
        deterministicRandom: true,
        testGroup: RexBenchGroup
    },
    // Simple
    {
        name: "async-fs",
        files: [
            "./simple/file-system.js"
        ],
        iterations: 40,
        worstCaseCount: 3,
        benchmarkClass: AsyncBenchmark,
        testGroup: SimpleGroup
    },
    {
        name: "float-mm.c",
        files: [
            "./simple/float-mm.c.js"
        ],
        iterations: 15,
        worstCaseCount: 2,
        testGroup: SimpleGroup
    },
    {
        name: "hash-map",
        files: [
            "./simple/hash-map.js"
        ],
        testGroup: SimpleGroup
    },
    // SeaMonster
    {
        name: "ai-astar",
        files: [
            "./SeaMonster/ai-astar.js"
        ],
        testGroup: SeaMonsterGroup
    },
    {
        name: "gaussian-blur",
        files: [
            "./SeaMonster/gaussian-blur.js"
        ],
        testGroup: SeaMonsterGroup
    },
    {
        name: "stanford-crypto-aes",
        files: [
            "./SeaMonster/sjlc.js"
            , "./SeaMonster/stanford-crypto-aes.js"
        ],
        testGroup: SeaMonsterGroup
    },
    {
        name: "stanford-crypto-pbkdf2",
        files: [
            "./SeaMonster/sjlc.js"
            , "./SeaMonster/stanford-crypto-pbkdf2.js"
        ],
        testGroup: SeaMonsterGroup
    },
    {
        name: "stanford-crypto-sha256",
        files: [
            "./SeaMonster/sjlc.js"
            , "./SeaMonster/stanford-crypto-sha256.js"
        ],
        testGroup: SeaMonsterGroup
    },
    {
        name: "json-stringify-inspector",
        files: [
            "./SeaMonster/inspector-json-payload.js"
            , "./SeaMonster/json-stringify-inspector.js"
        ],
        iterations: 20,
        worstCaseCount: 2,
        testGroup: SeaMonsterGroup
    },
    {
        name: "json-parse-inspector",
        files: [
            "./SeaMonster/inspector-json-payload.js"
            , "./SeaMonster/json-parse-inspector.js"
        ],
        iterations: 20,
        worstCaseCount: 2,
        testGroup: SeaMonsterGroup
    },
    // Wasm
    {
        name: "HashSet-wasm",
        wasmPath: "./wasm/HashSet.wasm",
        files: [
            "./wasm/HashSet.js"
        ],
        preload: {
            wasmBlobURL: "./wasm/HashSet.wasm"
        },
        benchmarkClass: WasmBenchmark,
        testGroup: WasmGroup
    },
    {
        name: "tsf-wasm",
        wasmPath: "./wasm/tsf.wasm",
        files: [
            "./wasm/tsf.js"
        ],
        preload: {
            wasmBlobURL: "./wasm/tsf.wasm"
        },
        benchmarkClass: WasmBenchmark,
        testGroup: WasmGroup
    },
    {
        name: "quicksort-wasm",
        wasmPath: "./wasm/quicksort.wasm",
        files: [
            "./wasm/quicksort.js"
        ],
        preload: {
            wasmBlobURL: "./wasm/quicksort.wasm"
        },
        benchmarkClass: WasmBenchmark,
        testGroup: WasmGroup
    },
    {
        name: "gcc-loops-wasm",
        wasmPath: "./wasm/gcc-loops.wasm",
        files: [
            "./wasm/gcc-loops.js"
        ],
        preload: {
            wasmBlobURL: "./wasm/gcc-loops.wasm"
        },
        benchmarkClass: WasmBenchmark,
        testGroup: WasmGroup
    },
    {
        name: "richards-wasm",
        wasmPath: "./wasm/richards.wasm",
        files: [
            "./wasm/richards.js"
        ],
        preload: {
            wasmBlobURL: "./wasm/richards.wasm"
        },
        benchmarkClass: WasmBenchmark,
        testGroup: WasmGroup
    },
    // WorkerTests
    {
        name: "bomb-workers",
        files: [
            "./worker/bomb.js"
        ],
        iterations: 80,
        preload: {
            rayTrace3D: "./worker/bomb-subtests/3d-raytrace.js"
            , accessNbody: "./worker/bomb-subtests/access-nbody.js"
            , morph3D: "./worker/bomb-subtests/3d-morph.js"
            , cube3D: "./worker/bomb-subtests/3d-cube.js"
            , accessFunnkuch: "./worker/bomb-subtests/access-fannkuch.js"
            , accessBinaryTrees: "./worker/bomb-subtests/access-binary-trees.js"
            , accessNsieve: "./worker/bomb-subtests/access-nsieve.js"
            , bitopsBitwiseAnd: "./worker/bomb-subtests/bitops-bitwise-and.js"
            , bitopsNsieveBits: "./worker/bomb-subtests/bitops-nsieve-bits.js"
            , controlflowRecursive: "./worker/bomb-subtests/controlflow-recursive.js"
            , bitops3BitBitsInByte: "./worker/bomb-subtests/bitops-3bit-bits-in-byte.js"
            , botopsBitsInByte: "./worker/bomb-subtests/bitops-bits-in-byte.js"
            , cryptoAES: "./worker/bomb-subtests/crypto-aes.js"
            , cryptoMD5: "./worker/bomb-subtests/crypto-md5.js"
            , cryptoSHA1: "./worker/bomb-subtests/crypto-sha1.js"
            , dateFormatTofte: "./worker/bomb-subtests/date-format-tofte.js"
            , dateFormatXparb: "./worker/bomb-subtests/date-format-xparb.js"
            , mathCordic: "./worker/bomb-subtests/math-cordic.js"
            , mathPartialSums: "./worker/bomb-subtests/math-partial-sums.js"
            , mathSpectralNorm: "./worker/bomb-subtests/math-spectral-norm.js"
            , stringBase64: "./worker/bomb-subtests/string-base64.js"
            , stringFasta: "./worker/bomb-subtests/string-fasta.js"
            , stringValidateInput: "./worker/bomb-subtests/string-validate-input.js"
            , stringTagcloud: "./worker/bomb-subtests/string-tagcloud.js"
            , stringUnpackCode: "./worker/bomb-subtests/string-unpack-code.js"
            , regexpDNA: "./worker/bomb-subtests/regexp-dna.js"
        },
        benchmarkClass: AsyncBenchmark,
        testGroup: WorkerTestsGroup
    },
    {
        name: "segmentation",
        files: [
            "./worker/segmentation.js"
        ],
        preload: {
            asyncTaskBlob: "./worker/async-task.js"
        },
        iterations: 36,
        worstCaseCount: 3,
        benchmarkClass: AsyncBenchmark,
        testGroup: WorkerTestsGroup
    },
    // WSL
    {
        name: "WSL",
        files: ["./WSL/Node.js" ,"./WSL/Type.js" ,"./WSL/ReferenceType.js" ,"./WSL/Value.js" ,"./WSL/Expression.js" ,"./WSL/Rewriter.js" ,"./WSL/Visitor.js" ,"./WSL/CreateLiteral.js" ,"./WSL/CreateLiteralType.js" ,"./WSL/PropertyAccessExpression.js" ,"./WSL/AddressSpace.js" ,"./WSL/AnonymousVariable.js" ,"./WSL/ArrayRefType.js" ,"./WSL/ArrayType.js" ,"./WSL/Assignment.js" ,"./WSL/AutoWrapper.js" ,"./WSL/Block.js" ,"./WSL/BoolLiteral.js" ,"./WSL/Break.js" ,"./WSL/CallExpression.js" ,"./WSL/CallFunction.js" ,"./WSL/Check.js" ,"./WSL/CheckLiteralTypes.js" ,"./WSL/CheckLoops.js" ,"./WSL/CheckRecursiveTypes.js" ,"./WSL/CheckRecursion.js" ,"./WSL/CheckReturns.js" ,"./WSL/CheckUnreachableCode.js" ,"./WSL/CheckWrapped.js" ,"./WSL/Checker.js" ,"./WSL/CloneProgram.js" ,"./WSL/CommaExpression.js" ,"./WSL/ConstexprFolder.js" ,"./WSL/ConstexprTypeParameter.js" ,"./WSL/Continue.js" ,"./WSL/ConvertPtrToArrayRefExpression.js" ,"./WSL/DereferenceExpression.js" ,"./WSL/DoWhileLoop.js" ,"./WSL/DotExpression.js" ,"./WSL/DoubleLiteral.js" ,"./WSL/DoubleLiteralType.js" ,"./WSL/EArrayRef.js" ,"./WSL/EBuffer.js" ,"./WSL/EBufferBuilder.js" ,"./WSL/EPtr.js" ,"./WSL/EnumLiteral.js" ,"./WSL/EnumMember.js" ,"./WSL/EnumType.js" ,"./WSL/EvaluationCommon.js" ,"./WSL/Evaluator.js" ,"./WSL/ExpressionFinder.js" ,"./WSL/ExternalOrigin.js" ,"./WSL/Field.js" ,"./WSL/FindHighZombies.js" ,"./WSL/FlattenProtocolExtends.js" ,"./WSL/FlattenedStructOffsetGatherer.js" ,"./WSL/FloatLiteral.js" ,"./WSL/FloatLiteralType.js" ,"./WSL/FoldConstexprs.js" ,"./WSL/ForLoop.js" ,"./WSL/Func.js" ,"./WSL/FuncDef.js" ,"./WSL/FuncInstantiator.js" ,"./WSL/FuncParameter.js" ,"./WSL/FunctionLikeBlock.js" ,"./WSL/HighZombieFinder.js" ,"./WSL/IdentityExpression.js" ,"./WSL/IfStatement.js" ,"./WSL/IndexExpression.js" ,"./WSL/InferTypesForCall.js" ,"./WSL/Inline.js" ,"./WSL/Inliner.js" ,"./WSL/InstantiateImmediates.js" ,"./WSL/IntLiteral.js" ,"./WSL/IntLiteralType.js" ,"./WSL/Intrinsics.js" ,"./WSL/LateChecker.js" ,"./WSL/Lexer.js" ,"./WSL/LexerToken.js" ,"./WSL/LiteralTypeChecker.js" ,"./WSL/LogicalExpression.js" ,"./WSL/LogicalNot.js" ,"./WSL/LoopChecker.js" ,"./WSL/MakeArrayRefExpression.js" ,"./WSL/MakePtrExpression.js" ,"./WSL/NameContext.js" ,"./WSL/NameFinder.js" ,"./WSL/NameResolver.js" ,"./WSL/NativeFunc.js" ,"./WSL/NativeFuncInstance.js" ,"./WSL/NativeType.js" ,"./WSL/NativeTypeInstance.js" ,"./WSL/NormalUsePropertyResolver.js" ,"./WSL/NullLiteral.js" ,"./WSL/NullType.js" ,"./WSL/OriginKind.js" ,"./WSL/OverloadResolutionFailure.js" ,"./WSL/Parse.js" ,"./WSL/Prepare.js" ,"./WSL/Program.js" ,"./WSL/ProgramWithUnnecessaryThingsRemoved.js" ,"./WSL/PropertyResolver.js" ,"./WSL/Protocol.js" ,"./WSL/ProtocolDecl.js" ,"./WSL/ProtocolFuncDecl.js" ,"./WSL/ProtocolRef.js" ,"./WSL/PtrType.js" ,"./WSL/ReadModifyWriteExpression.js" ,"./WSL/RecursionChecker.js" ,"./WSL/RecursiveTypeChecker.js" ,"./WSL/ResolveNames.js" ,"./WSL/ResolveOverloadImpl.js" ,"./WSL/ResolveProperties.js" ,"./WSL/ResolveTypeDefs.js" ,"./WSL/Return.js" ,"./WSL/ReturnChecker.js" ,"./WSL/ReturnException.js" ,"./WSL/StandardLibrary.js" ,"./WSL/StatementCloner.js" ,"./WSL/StructLayoutBuilder.js" ,"./WSL/StructType.js" ,"./WSL/Substitution.js" ,"./WSL/SwitchCase.js" ,"./WSL/SwitchStatement.js" ,"./WSL/SynthesizeEnumFunctions.js" ,"./WSL/SynthesizeStructAccessors.js" ,"./WSL/TrapStatement.js" ,"./WSL/TypeDef.js" ,"./WSL/TypeDefResolver.js" ,"./WSL/TypeOrVariableRef.js" ,"./WSL/TypeParameterRewriter.js" ,"./WSL/TypeRef.js" ,"./WSL/TypeVariable.js" ,"./WSL/TypeVariableTracker.js" ,"./WSL/TypedValue.js" ,"./WSL/UintLiteral.js" ,"./WSL/UintLiteralType.js" ,"./WSL/UnificationContext.js" ,"./WSL/UnreachableCodeChecker.js" ,"./WSL/VariableDecl.js" ,"./WSL/VariableRef.js" ,"./WSL/VisitingSet.js" ,"./WSL/WSyntaxError.js" ,"./WSL/WTrapError.js" ,"./WSL/WTypeError.js" ,"./WSL/WhileLoop.js" ,"./WSL/WrapChecker.js", "./WSL/Test.js"],
        benchmarkClass: WSLBenchmark,
        testGroup: WSLGroup
    }
];

// LuaJSFight tests
let luaJSFightTests = [
    "hello_world"
    , "list_search"
    , "lists"
    , "string_lists"
];
for (let test of luaJSFightTests) {
    testPlans.push({
        name: `${test}-LJF`,
        files: [
            `./LuaJSFight/${test}.js`
        ],
        testGroup: LuaJSFightGroup
    });
}

// SunSpider tests
let sunSpiderTests = [
    "3d-cube"
    , "3d-raytrace"
    , "base64"
    , "crypto-aes"
    , "crypto-md5"
    , "crypto-sha1"
    , "date-format-tofte"
    , "date-format-xparb"
    , "n-body"
    , "regex-dna"
    , "string-unpack-code"
    , "tagcloud"
];
for (let test of sunSpiderTests) {
    testPlans.push({
        name: `${test}-SP`,
        files: [
            `./SunSpider/${test}.js`
        ],
        testGroup: SunSpiderGroup
    });
}

// WTB (Web Tooling Benchmark) tests
let WTBTests = [
    "acorn"
    , "babylon"
    , "chai"
    , "coffeescript"
    , "espree"
    , "jshint"
    , "lebab"
    , "prepack"
    , "uglify-js"
];
for (let name of WTBTests) {
    testPlans.push({
        name: `${name}-wtb`,
        files: [
            isInBrowser ? "./web-tooling-benchmark/browser.js" : "./web-tooling-benchmark/cli.js"
            , `./web-tooling-benchmark/${name}.js`
        ],
        iterations: 5,
        worstCaseCount: 1,
        testGroup: WTBGroup
    });
}


let testsByName = new Map();
let testsByGroup = new Map();

for (let plan of testPlans) {
    let testName = plan.name;

    if (testsByName.has(plan.name))
        throw "Duplicate test plan with name \"" + testName + "\"";
    else
        testsByName.set(testName, plan);

    let group = plan.testGroup;

    if (testsByGroup.has(group))
        testsByGroup.get(group).push(testName);
    else
        testsByGroup.set(group, [testName]);
}

var JetStream = new Driver();

function addTestByName(testName)
{
    let plan = testsByName.get(testName);

    if (plan)
        JetStream.addPlan(plan, plan.benchmarkClass);
    else
        throw "Couldn't find test named \"" +  testName + "\"";
}

function addTestsByGroup(group)
{
    let testList = testsByGroup.get(group);

    if (!testList)
        throw "Couldn't find test group named: \"" + Symbol.keyFor(group) + "\"";

    for (let testName of testList)
        addTestByName(testName);
}

function processTestList(testList)
{
    let tests = [];

    if (testList instanceof Array)
        tests = testList;
    else
        tests = testList.split(/[\s,]/);

    for (let testName of tests) {
        let groupTest = testsByGroup.get(Symbol.for(testName));

        if (groupTest) {
            for (let testName of groupTest)
                addTestByName(testName);
        } else
            addTestByName(testName);
    }
}

let runOctane = true;
let runARES = true;
let runWSL = true;
let runRexBench = true;
let runWTB = true;
let runSunSpider = true;
let runSimple = true;
let runCDJS = true;
let runWorkerTests = !!isInBrowser;
let runSeaMonster = true;
let runCodeLoad = true;
let runWasm = true;

if (false) {
    runOctane = false;
    runARES = false;
    runWSL = false;
    runRexBench = false;
    runWTB = false;
    runSunSpider = false;
    runSimple = false;
    runCDJS = false;
    runWorkerTests = false;
    runSeaMonster = false;
    runCodeLoad = false;
    runWasm = false;
}

if (typeof testList !== "undefined") {
    processTestList(testList);
} else {
    if (runARES)
        addTestsByGroup(ARESGroup);

    if (runCDJS)
        addTestsByGroup(CDJSGroup);

    if (runCodeLoad)
        addTestsByGroup(CodeLoadGroup);

    if (runOctane)
        addTestsByGroup(OctaneGroup);

    if (runRexBench)
        addTestsByGroup(RexBenchGroup);

    if (runSeaMonster)
        addTestsByGroup(SeaMonsterGroup);

    if (runSimple)
        addTestsByGroup(SimpleGroup);

    if (runSunSpider)
        addTestsByGroup(SunSpiderGroup);

    if (runWasm)
        addTestsByGroup(WasmGroup);

    if (runWorkerTests)
        addTestsByGroup(WorkerTestsGroup);

    if (runWSL)
        addTestsByGroup(WSLGroup);

    if (runWTB)
        addTestsByGroup(WTBGroup);
}


JetStream.initialize()
    .then(() => JetStream.start())
    .catch((error) => print("JetStream2 failed:", error && error.stack ? error.stack : error));
undefined;
