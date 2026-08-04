
const isInBrowser = false;
const isD8 = false;
const isSpiderMonkey = false;
const jetStreamHostPrint = typeof globalThis.print === "function"
    ? globalThis.print
    : (...args) => globalThis.console.log(...args);
globalThis.print = jetStreamHostPrint;
var __jetstreamPhase = (phase) =>
    jetStreamHostPrint("JETSTREAM_PHASE:" + Date.now() + ":" + phase);
var console = {
    log: (...args) => jetStreamHostPrint(...args),
    warn: (...args) => jetStreamHostPrint(...args),
    error: (...args) => jetStreamHostPrint(...args),
    assert(condition, ...args) {
        if (!condition)
            throw new Error(args.join(" ") || "Assertion failed");
    },
};
var runString = () => {
    globalThis.loadString = (source) =>
        new Function("top", source)(globalThis.top);
    return globalThis;
};
var load = (name) => globalThis.loadString(readFile(name));
var performance = globalThis.performance = {
    now: Date.now.bind(Date),
    mark(name) { return { name }; },
    measure() {},
};
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
    testIterationCount: 1,
    testWorstCaseCount: 0,
    testIterationCountMap: {},
    testWorstCaseCountMap: {},
    testList: "cdjs",
};
var __jetstreamResources = {"./cdjs/benchmark.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\n// Copyright (C) 2015-2016 Apple Inc. All rights reserved.\n// \n// Redistribution and use in source and binary forms, with or without\n// modification, are permitted provided that the following conditions are met:\n//  * Redistributions of source code must retain the above copyright\n//    notice, this list of conditions and the following disclaimer.\n//  * Redistributions in binary form must reproduce the above copyright\n//    notice, this list of conditions and the following disclaimer in the\n//    documentation and/or other materials provided with the distribution.\n//  * Neither the name of the Purdue University nor the\n//    names of its contributors may be used to endorse or promote products\n//    derived from this software without specific prior written permission.\n// \n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\n\nfunction benchmarkImpl(configuration) {\n    var verbosity = configuration.verbosity;\n    var numAircraft = configuration.numAircraft;\n    var numFrames = configuration.numFrames;\n    var expectedCollisions = configuration.expectedCollisions;\n    var exclude = configuration.exclude;\n\n    var simulator = new Simulator(numAircraft);\n    var detector = new CollisionDetector();\n    var lastTime = performance.now();\n    var results = [];\n    for (var i = 0; i < numFrames; ++i) {\n        var time = i / 10;\n        \n        var collisions = detector.handleNewFrame(simulator.simulate(time));\n        \n        var before = lastTime;\n        var after = performance.now();\n        lastTime = after;\n        var result = {\n            time: after - before,\n            numCollisions: collisions.length\n        };\n        if (verbosity >= 2)\n            print(\"CDjs: \" + result.time);\n        if (verbosity >= 3)\n            result.collisions = collisions;\n        results.push(result);\n    }\n    \n    results.splice(0, exclude);\n\n    if (verbosity >= 1) {\n        for (var i = 0; i < results.length; ++i) {\n            var string = \"Frame \" + i + \": \" + results[i].time + \" ms.\";\n            if (results[i].numCollisions)\n                string += \" (\" + results[i].numCollisions + \" collisions.)\";\n            print(string);\n            if (verbosity >= 2 && results[i].collisions.length)\n                print(\"    Collisions: \" + results[i].collisions);\n        }\n    }\n\n    // Check results.\n    var actualCollisions = 0;\n    for (var i = 0; i < results.length; ++i)\n        actualCollisions += results[i].numCollisions;\n    if (actualCollisions != expectedCollisions) {\n        throw new Error(\"Bad number of collisions: \" + actualCollisions + \" (expected \" + expectedCollisions + \")\");\n    }\n}\n\nfunction benchmark() {\n    return benchmarkImpl({\n        verbosity: 0,\n        numAircraft: 1000,\n        numFrames: 18,\n        expectedCollisions: 1336,\n        exclude: 0\n    });\n}\n\nclass Benchmark {\n    runIteration() {\n        benchmark();\n    }\n}\n","./cdjs/call_sign.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\n// Copyright (C) 2015 Apple Inc. All rights reserved.\n// \n// Redistribution and use in source and binary forms, with or without\n// modification, are permitted provided that the following conditions are met:\n//  * Redistributions of source code must retain the above copyright\n//    notice, this list of conditions and the following disclaimer.\n//  * Redistributions in binary form must reproduce the above copyright\n//    notice, this list of conditions and the following disclaimer in the\n//    documentation and/or other materials provided with the distribution.\n//  * Neither the name of the Purdue University nor the\n//    names of its contributors may be used to endorse or promote products\n//    derived from this software without specific prior written permission.\n// \n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\n\nfunction CallSign(value) {\n    this._value = value;\n}\n\nCallSign.prototype.compareTo = function(other) {\n    return this._value.localeCompare(other._value);\n}\n\nCallSign.prototype.toString = function() {\n    return this._value;\n}\n\n","./cdjs/collision.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\n// Copyright (C) 2015 Apple Inc. All rights reserved.\n// \n// Redistribution and use in source and binary forms, with or without\n// modification, are permitted provided that the following conditions are met:\n//  * Redistributions of source code must retain the above copyright\n//    notice, this list of conditions and the following disclaimer.\n//  * Redistributions in binary form must reproduce the above copyright\n//    notice, this list of conditions and the following disclaimer in the\n//    documentation and/or other materials provided with the distribution.\n//  * Neither the name of the Purdue University nor the\n//    names of its contributors may be used to endorse or promote products\n//    derived from this software without specific prior written permission.\n// \n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\n\nfunction Collision(aircraft, position) {\n    this.aircraft = aircraft;\n    this.position = position;\n}\n\nCollision.prototype.toString = function() {\n    return \"Collision(\" + this.aircraft + \" at \" + this.position + \")\";\n};\n\n","./cdjs/collision_detector.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\n// Copyright (C) 2015 Apple Inc. All rights reserved.\n// \n// Redistribution and use in source and binary forms, with or without\n// modification, are permitted provided that the following conditions are met:\n//  * Redistributions of source code must retain the above copyright\n//    notice, this list of conditions and the following disclaimer.\n//  * Redistributions in binary form must reproduce the above copyright\n//    notice, this list of conditions and the following disclaimer in the\n//    documentation and/or other materials provided with the distribution.\n//  * Neither the name of the Purdue University nor the\n//    names of its contributors may be used to endorse or promote products\n//    derived from this software without specific prior written permission.\n// \n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\n\nfunction CollisionDetector() {\n    this._state = new RedBlackTree();\n}\n\nCollisionDetector.prototype.handleNewFrame = function(frame) {\n    var motions = [];\n    var seen = new RedBlackTree();\n    \n    for (var i = 0; i < frame.length; ++i) {\n        var aircraft = frame[i];\n        \n        var oldPosition = this._state.put(aircraft.callsign, aircraft.position);\n        var newPosition = aircraft.position;\n        seen.put(aircraft.callsign, true);\n        \n        if (!oldPosition) {\n            // Treat newly introduced aircraft as if they were stationary.\n            oldPosition = newPosition;\n        }\n        \n        motions.push(new Motion(aircraft.callsign, oldPosition, newPosition));\n    }\n    \n    // Remove aircraft that are no longer present.\n    var toRemove = [];\n    this._state.forEach(function(callsign, position) {\n        if (!seen.get(callsign))\n            toRemove.push(callsign);\n    });\n    for (var i = 0; i < toRemove.length; ++i)\n        this._state.remove(toRemove[i]);\n    \n    var allReduced = reduceCollisionSet(motions);\n    var collisions = [];\n    for (var reductionIndex = 0; reductionIndex < allReduced.length; ++reductionIndex) {\n        var reduced = allReduced[reductionIndex];\n        for (var i = 0; i < reduced.length; ++i) {\n            var motion1 = reduced[i];\n            for (var j = i + 1; j < reduced.length; ++j) {\n                var motion2 = reduced[j];\n                var collision = motion1.findIntersection(motion2);\n                if (collision)\n                    collisions.push(new Collision([motion1.callsign, motion2.callsign], collision));\n            }\n        }\n    }\n    \n    return collisions;\n};\n","./cdjs/constants.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\n// Copyright (C) 2015 Apple Inc. All rights reserved.\n// \n// Redistribution and use in source and binary forms, with or without\n// modification, are permitted provided that the following conditions are met:\n//  * Redistributions of source code must retain the above copyright\n//    notice, this list of conditions and the following disclaimer.\n//  * Redistributions in binary form must reproduce the above copyright\n//    notice, this list of conditions and the following disclaimer in the\n//    documentation and/or other materials provided with the distribution.\n//  * Neither the name of the Purdue University nor the\n//    names of its contributors may be used to endorse or promote products\n//    derived from this software without specific prior written permission.\n// \n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\n\nvar Constants = {};\nConstants.MIN_X = 0;\nConstants.MIN_Y = 0;\nConstants.MAX_X = 1000;\nConstants.MAX_Y = 1000;\nConstants.MIN_Z = 0;\nConstants.MAX_Z = 10;\nConstants.PROXIMITY_RADIUS = 1;\nConstants.GOOD_VOXEL_SIZE = Constants.PROXIMITY_RADIUS * 2;\n","./cdjs/motion.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\n// Copyright (C) 2015 Apple Inc. All rights reserved.\n// \n// Redistribution and use in source and binary forms, with or without\n// modification, are permitted provided that the following conditions are met:\n//  * Redistributions of source code must retain the above copyright\n//    notice, this list of conditions and the following disclaimer.\n//  * Redistributions in binary form must reproduce the above copyright\n//    notice, this list of conditions and the following disclaimer in the\n//    documentation and/or other materials provided with the distribution.\n//  * Neither the name of the Purdue University nor the\n//    names of its contributors may be used to endorse or promote products\n//    derived from this software without specific prior written permission.\n// \n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\n\nfunction Motion(callsign, posOne, posTwo) {\n    this.callsign = callsign;\n    this.posOne = posOne;\n    this.posTwo = posTwo;\n}\n\nMotion.prototype.toString = function() {\n    return \"Motion(\" + this.callsign + \" from \" + this.posOne + \" to \" + this.posTwo + \")\";\n};\n\nMotion.prototype.delta = function() {\n    return this.posTwo.minus(this.posOne);\n};\n\nMotion.prototype.findIntersection = function(other) {\n    var init1 = this.posOne;\n    var init2 = other.posOne;\n    var vec1 = this.delta();\n    var vec2 = other.delta();\n    var radius = Constants.PROXIMITY_RADIUS;\n    \n    // this test is not geometrical 3-d intersection test, it takes the fact that the aircraft move\n    // into account ; so it is more like a 4d test\n    // (it assumes that both of the aircraft have a constant speed over the tested interval)\n    \n    // we thus have two points, each of them moving on its line segment at constant speed ; we are looking\n    // for times when the distance between these two points is smaller than r \n    \n    // vec1 is vector of aircraft 1\n    // vec2 is vector of aircraft 2\n    \n    // a = (V2 - V1)^T * (V2 - V1)\n    var a = vec2.minus(vec1).squaredMagnitude();\n    \n    if (a != 0) {\n        // we are first looking for instances of time when the planes are exactly r from each other\n        // at least one plane is moving ; if the planes are moving in parallel, they do not have constant speed\n\n        // if the planes are moving in parallel, then\n        //   if the faster starts behind the slower, we can have 2, 1, or 0 solutions\n        //   if the faster plane starts in front of the slower, we can have 0 or 1 solutions\n\n        // if the planes are not moving in parallel, then\n\n\n        // point P1 = I1 + vV1\n        // point P2 = I2 + vV2\n        //   - looking for v, such that dist(P1,P2) = || P1 - P2 || = r\n\n        // it follows that || P1 - P2 || = sqrt( < P1-P2, P1-P2 > )\n        //   0 = -r^2 + < P1 - P2, P1 - P2 >\n        //  from properties of dot product\n        //   0 = -r^2 + <I1-I2,I1-I2> + v * 2<I1-I2, V1-V2> + v^2 *<V1-V2,V1-V2>\n        //   so we calculate a, b, c - and solve the quadratic equation\n        //   0 = c + bv + av^2\n\n        // b = 2 * <I1-I2, V1-V2>\n\n        var b = 2 * init1.minus(init2).dot(vec1.minus(vec2));\n\n        // c = -r^2 + (I2 - I1)^T * (I2 - I1)\n        var c = -radius * radius + init2.minus(init1).squaredMagnitude();\n\n        var discr = b * b - 4 * a * c;\n        if (discr < 0)\n            return null;\n\n        var v1 = (-b - Math.sqrt(discr)) / (2 * a);\n        var v2 = (-b + Math.sqrt(discr)) / (2 * a);\n\n        if (v1 <= v2 && ((v1 <= 1 && 1 <= v2) ||\n                         (v1 <= 0 && 0 <= v2) ||\n                         (0 <= v1 && v2 <= 1))) {\n            // Pick a good \"time\" at which to report the collision.\n            var v;\n            if (v1 <= 0) {\n                // The collision started before this frame. Report it at the start of the frame.\n                v = 0;\n            } else {\n                // The collision started during this frame. Report it at that moment.\n                v = v1;\n            }\n            \n            var result1 = init1.plus(vec1.times(v));\n            var result2 = init2.plus(vec2.times(v));\n            \n            var result = result1.plus(result2).times(0.5);\n            if (result.x >= Constants.MIN_X &&\n                result.x <= Constants.MAX_X &&\n                result.y >= Constants.MIN_Y &&\n                result.y <= Constants.MAX_Y &&\n                result.z >= Constants.MIN_Z &&\n                result.z <= Constants.MAX_Z)\n                return result;\n        }\n\n        return null;\n    }\n    \n    // the planes have the same speeds and are moving in parallel (or they are not moving at all)\n    // they  thus have the same distance all the time ; we calculate it from the initial point\n    \n    // dist = || i2 - i1 || = sqrt(  ( i2 - i1 )^T * ( i2 - i1 ) )\n    \n    var dist = init2.minus(init1).magnitude();\n    if (dist <= radius)\n        return init1.plus(init2).times(0.5);\n    \n    return null;\n};\n\n","./cdjs/red_black_tree.js":"/*\n * Copyright (C) 2010, 2011, 2015 Apple Inc. All rights reserved.\n *\n * Redistribution and use in source and binary forms, with or without\n * modification, are permitted provided that the following conditions\n * are met:\n *\n * 1.  Redistributions of source code must retain the above copyright\n *     notice, this list of conditions and the following disclaimer.\n * 2.  Redistributions in binary form must reproduce the above copyright\n *     notice, this list of conditions and the following disclaimer in the\n *     documentation and/or other materials provided with the distribution.\n * 3.  Neither the name of Apple Inc. (\"Apple\") nor the names of\n *     its contributors may be used to endorse or promote products derived\n *     from this software without specific prior written permission.\n *\n * THIS SOFTWARE IS PROVIDED BY APPLE AND ITS CONTRIBUTORS \"AS IS\" AND ANY\n * EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\n * WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\n * DISCLAIMED. IN NO EVENT SHALL APPLE OR ITS CONTRIBUTORS BE LIABLE FOR ANY\n * DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\n * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\n * LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\n * ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\n * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF\n * THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\n */\n\nvar RedBlackTree = (function(){\n    function compare(a, b) {\n        return a.compareTo(b);\n    }\n    \n    function treeMinimum(x) {\n        while (x.left)\n            x = x.left;\n        return x;\n    }\n    \n    function treeMaximum(x) {\n        while (x.right)\n            x = x.right;\n        return x;\n    }\n    \n    function Node(key, value) {\n        this.key = key;\n        this.value = value;\n        this.left = null;\n        this.right = null;\n        this.parent = null;\n        this.color = \"red\";\n    }\n    \n    Node.prototype.successor = function() {\n        var x = this;\n        if (x.right)\n            return treeMinimum(x.right);\n        var y = x.parent;\n        while (y && x == y.right) {\n            x = y;\n            y = y.parent;\n        }\n        return y;\n    };\n    \n    Node.prototype.predecessor = function() {\n        var x = this;\n        if (x.left)\n            return treeMaximum(x.left);\n        var y = x.parent;\n        while (y && x == y.left) {\n            x = y;\n            y = y.parent;\n        }\n        return y;\n    };\n    \n    Node.prototype.toString = function() {\n        return this.key + \"=>\" + this.value + \" (\" + this.color + \")\";\n    };\n    \n    function RedBlackTree() {\n        this._root = null;\n    }\n    \n    RedBlackTree.prototype.put = function(key, value) {\n        var insertionResult = this._treeInsert(key, value);\n        if (!insertionResult.isNewEntry)\n            return insertionResult.oldValue;\n        var x = insertionResult.newNode;\n        \n        while (x != this._root && x.parent.color == \"red\") {\n            if (x.parent == x.parent.parent.left) {\n                var y = x.parent.parent.right;\n                if (y && y.color == \"red\") {\n                    // Case 1\n                    x.parent.color = \"black\";\n                    y.color = \"black\";\n                    x.parent.parent.color = \"red\";\n                    x = x.parent.parent;\n                } else {\n                    if (x == x.parent.right) {\n                        // Case 2\n                        x = x.parent;\n                        this._leftRotate(x);\n                    }\n                    // Case 3\n                    x.parent.color = \"black\";\n                    x.parent.parent.color = \"red\";\n                    this._rightRotate(x.parent.parent);\n                }\n            } else {\n                // Same as \"then\" clause with \"right\" and \"left\" exchanged.\n                var y = x.parent.parent.left;\n                if (y && y.color == \"red\") {\n                    // Case 1\n                    x.parent.color = \"black\";\n                    y.color = \"black\";\n                    x.parent.parent.color = \"red\";\n                    x = x.parent.parent;\n                } else {\n                    if (x == x.parent.left) {\n                        // Case 2\n                        x = x.parent;\n                        this._rightRotate(x);\n                    }\n                    // Case 3\n                    x.parent.color = \"black\";\n                    x.parent.parent.color = \"red\";\n                    this._leftRotate(x.parent.parent);\n                }\n            }\n        }\n        \n        this._root.color = \"black\";\n        return null;\n    };\n    \n    RedBlackTree.prototype.remove = function(key) {\n        var z = this._findNode(key);\n        if (!z)\n            return null;\n        \n        // Y is the node to be unlinked from the tree.\n        var y;\n        if (!z.left || !z.right)\n            y = z;\n        else\n            y = z.successor();\n\n        // Y is guaranteed to be non-null at this point.\n        var x;\n        if (y.left)\n            x = y.left;\n        else\n            x = y.right;\n        \n        // X is the child of y which might potentially replace y in the tree. X might be null at\n        // this point.\n        var xParent;\n        if (x) {\n            x.parent = y.parent;\n            xParent = x.parent;\n        } else\n            xParent = y.parent;\n        if (!y.parent)\n            this._root = x;\n        else {\n            if (y == y.parent.left)\n                y.parent.left = x;\n            else\n                y.parent.right = x;\n        }\n        \n        if (y != z) {\n            if (y.color == \"black\")\n                this._removeFixup(x, xParent);\n            \n            y.parent = z.parent;\n            y.color = z.color;\n            y.left = z.left;\n            y.right = z.right;\n            \n            if (z.left)\n                z.left.parent = y;\n            if (z.right)\n                z.right.parent = y;\n            if (z.parent) {\n                if (z.parent.left == z)\n                    z.parent.left = y;\n                else\n                    z.parent.right = y;\n            } else\n                this._root = y;\n        } else if (y.color == \"black\")\n            this._removeFixup(x, xParent);\n        \n        return z.value;\n    };\n    \n    RedBlackTree.prototype.get = function(key) {\n        var node = this._findNode(key);\n        if (!node)\n            return null;\n        return node.value;\n    };\n    \n    RedBlackTree.prototype.forEach = function(callback) {\n        if (!this._root)\n            return;\n        for (var current = treeMinimum(this._root); current; current = current.successor())\n            callback(current.key, current.value);\n    };\n    \n    RedBlackTree.prototype.asArray = function() {\n        var result = [];\n        this.forEach(function(key, value) {\n            result.push({key: key, value: value});\n        });\n        return result;\n    };\n    \n    RedBlackTree.prototype.toString = function() {\n        var result = \"[\";\n        var first = true;\n        this.forEach(function(key, value) {\n            if (first)\n                first = false;\n            else\n                result += \", \";\n            result += key + \"=>\" + value;\n        });\n        return result + \"]\";\n    };\n    \n    RedBlackTree.prototype._findNode = function(key) {\n        for (var current = this._root; current;) {\n            var comparisonResult = compare(key, current.key);\n            if (!comparisonResult)\n                return current;\n            if (comparisonResult < 0)\n                current = current.left;\n            else\n                current = current.right;\n        }\n        return null;\n    };\n    \n    RedBlackTree.prototype._treeInsert = function(key, value) {\n        var y = null;\n        var x = this._root;\n        while (x) {\n            y = x;\n            var comparisonResult = key.compareTo(x.key);\n            if (comparisonResult < 0)\n                x = x.left;\n            else if (comparisonResult > 0)\n                x = x.right;\n            else {\n                var oldValue = x.value;\n                x.value = value;\n                return {isNewEntry:false, oldValue:oldValue};\n            }\n        }\n        var z = new Node(key, value);\n        z.parent = y;\n        if (!y)\n            this._root = z;\n        else {\n            if (key.compareTo(y.key) < 0)\n                y.left = z;\n            else\n                y.right = z;\n        }\n        return {isNewEntry:true, newNode:z};\n    };\n    \n    RedBlackTree.prototype._leftRotate = function(x) {\n        var y = x.right;\n        \n        // Turn y's left subtree into x's right subtree.\n        x.right = y.left;\n        if (y.left)\n            y.left.parent = x;\n        \n        // Link x's parent to y.\n        y.parent = x.parent;\n        if (!x.parent)\n            this._root = y;\n        else {\n            if (x == x.parent.left)\n                x.parent.left = y;\n            else\n                x.parent.right = y;\n        }\n        \n        // Put x on y's left.\n        y.left = x;\n        x.parent = y;\n        \n        return y;\n    };\n    \n    RedBlackTree.prototype._rightRotate = function(y) {\n        var x = y.left;\n        \n        // Turn x's right subtree into y's left subtree.\n        y.left = x.right;\n        if (x.right)\n            x.right.parent = y;\n        \n        // Link y's parent to x;\n        x.parent = y.parent;\n        if (!y.parent)\n            this._root = x;\n        else {\n            if (y == y.parent.left)\n                y.parent.left = x;\n            else\n                y.parent.right = x;\n        }\n        \n        x.right = y;\n        y.parent = x;\n        \n        return x;\n    };\n    \n    RedBlackTree.prototype._removeFixup = function(x, xParent) {\n        while (x != this._root && (!x || x.color == \"black\")) {\n            if (x == xParent.left) {\n                // Note: the text points out that w cannot be null. The reason is not obvious from\n                // simply looking at the code; it comes about from the properties of the red-black\n                // tree.\n                var w = xParent.right;\n                if (w.color == \"red\") {\n                    // Case 1\n                    w.color = \"black\";\n                    xParent.color = \"red\";\n                    this._leftRotate(xParent);\n                    w = xParent.right;\n                }\n                if ((!w.left || w.left.color == \"black\")\n                    && (!w.right || w.right.color == \"black\")) {\n                    // Case 2\n                    w.color = \"red\";\n                    x = xParent;\n                    xParent = x.parent;\n                } else {\n                    if (!w.right || w.right.color == \"black\") {\n                        // Case 3\n                        w.left.color = \"black\";\n                        w.color = \"red\";\n                        this._rightRotate(w);\n                        w = xParent.right;\n                    }\n                    // Case 4\n                    w.color = xParent.color;\n                    xParent.color = \"black\";\n                    if (w.right)\n                        w.right.color = \"black\";\n                    this._leftRotate(xParent);\n                    x = this._root;\n                    xParent = x.parent;\n                }\n            } else {\n                // Same as \"then\" clause with \"right\" and \"left\" exchanged.\n                \n                var w = xParent.left;\n                if (w.color == \"red\") {\n                    // Case 1\n                    w.color = \"black\";\n                    xParent.color = \"red\";\n                    this._rightRotate(xParent);\n                    w = xParent.left;\n                }\n                if ((!w.right || w.right.color == \"black\")\n                    && (!w.left || w.left.color == \"black\")) {\n                    // Case 2\n                    w.color = \"red\";\n                    x = xParent;\n                    xParent = x.parent;\n                } else {\n                    if (!w.left || w.left.color == \"black\") {\n                        // Case 3\n                        w.right.color = \"black\";\n                        w.color = \"red\";\n                        this._leftRotate(w);\n                        w = xParent.left;\n                    }\n                    // Case 4\n                    w.color = xParent.color;\n                    xParent.color = \"black\";\n                    if (w.left)\n                        w.left.color = \"black\";\n                    this._rightRotate(xParent);\n                    x = this._root;\n                    xParent = x.parent;\n                }\n            }\n        }\n        if (x)\n            x.color = \"black\";\n    };\n    \n    return RedBlackTree;\n})();\n\n","./cdjs/reduce_collision_set.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\n// Copyright (C) 2015 Apple Inc. All rights reserved.\n// \n// Redistribution and use in source and binary forms, with or without\n// modification, are permitted provided that the following conditions are met:\n//  * Redistributions of source code must retain the above copyright\n//    notice, this list of conditions and the following disclaimer.\n//  * Redistributions in binary form must reproduce the above copyright\n//    notice, this list of conditions and the following disclaimer in the\n//    documentation and/or other materials provided with the distribution.\n//  * Neither the name of the Purdue University nor the\n//    names of its contributors may be used to endorse or promote products\n//    derived from this software without specific prior written permission.\n// \n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\n\nvar drawMotionOnVoxelMap = (function() {\n    var voxelSize = Constants.GOOD_VOXEL_SIZE;\n    var horizontal = new Vector2D(voxelSize, 0);\n    var vertical = new Vector2D(0, voxelSize);\n    \n    function voxelHash(position) {\n        var xDiv = (position.x / voxelSize) | 0;\n        var yDiv = (position.y / voxelSize) | 0;\n        \n        var result = new Vector2D();\n        result.x = voxelSize * xDiv;\n        result.y = voxelSize * yDiv;\n        \n        if (position.x < 0)\n            result.x -= voxelSize;\n        if (position.y < 0)\n            result.y -= voxelSize;\n        \n        return result;\n    }\n    \n    return function(voxelMap, motion) {\n        var seen = new RedBlackTree();\n        \n        function putIntoMap(voxel) {\n            var array = voxelMap.get(voxel);\n            if (!array)\n                voxelMap.put(voxel, array = []);\n            array.push(motion);\n        }\n        \n        function isInVoxel(voxel) {\n            if (voxel.x > Constants.MAX_X ||\n                voxel.x < Constants.MIN_X ||\n                voxel.y > Constants.MAX_Y ||\n                voxel.y < Constants.MIN_Y)\n                return false;\n            \n            var init = motion.posOne;\n            var fin = motion.posTwo;\n            \n            var v_s = voxelSize;\n            var r = Constants.PROXIMITY_RADIUS / 2;\n            \n            var v_x = voxel.x;\n            var x0 = init.x;\n            var xv = fin.x - init.x;\n            \n            var v_y = voxel.y;\n            var y0 = init.y;\n            var yv = fin.y - init.y;\n            \n            var low_x, high_x;\n            low_x = (v_x - r - x0) / xv;\n            high_x = (v_x + v_s + r - x0) / xv;\n            \n            if (xv < 0) {\n                var tmp = low_x;\n                low_x = high_x;\n                high_x = tmp;\n            }\n            \n            var low_y, high_y;\n            low_y = (v_y - r - y0) / yv;\n            high_y = (v_y + v_s + r - y0) / yv;\n            \n            if (yv < 0) {\n                var tmp = low_y;\n                low_y = high_y;\n                high_y = tmp;\n            }\n            \n            if (false) {\n                print(\"v_x = \" + v_x + \", x0 = \" + x0 + \", xv = \" + xv + \", v_y = \" + v_y + \", y0 = \" + y0 + \", yv = \" + yv + \", low_x = \" + low_x + \", low_y = \" + low_y + \", high_x = \" + high_x + \", high_y = \" + high_y);\n            }\n            \n            return (((xv == 0 && v_x <= x0 + r && x0 - r <= v_x + v_s) /* no motion in x */ || \n                     ((low_x <= 1 && 1 <= high_x) || (low_x <= 0 && 0 <= high_x) ||\n                      (0 <= low_x && high_x <= 1))) && \n                    ((yv == 0 && v_y <= y0 + r && y0 - r <= v_y + v_s) /* no motion in y */ || \n                     ((low_y <= 1 && 1 <= high_y) || (low_y <= 0 && 0 <= high_y) ||\n                      (0 <= low_y && high_y <= 1))) && \n                    (xv == 0 || yv == 0 || /* no motion in x or y or both */\n                     (low_y <= high_x && high_x <= high_y) ||\n                     (low_y <= low_x && low_x <= high_y) ||\n                     (low_x <= low_y && high_y <= high_x)));\n        }\n        \n        function recurse(nextVoxel) {\n            if (!isInVoxel(nextVoxel, motion))\n                return;\n            if (seen.put(nextVoxel, true))\n                return;\n            \n            putIntoMap(nextVoxel);\n            \n            recurse(nextVoxel.minus(horizontal));\n            recurse(nextVoxel.plus(horizontal));\n            recurse(nextVoxel.minus(vertical));\n            recurse(nextVoxel.plus(vertical));\n            recurse(nextVoxel.minus(horizontal).minus(vertical));\n            recurse(nextVoxel.minus(horizontal).plus(vertical));\n            recurse(nextVoxel.plus(horizontal).minus(vertical));\n            recurse(nextVoxel.plus(horizontal).plus(vertical));\n        }\n        \n        recurse(voxelHash(motion.posOne));\n    };\n})();\n\nfunction reduceCollisionSet(motions) {\n    var voxelMap = new RedBlackTree();\n    for (var i = 0; i < motions.length; ++i)\n        drawMotionOnVoxelMap(voxelMap, motions[i]);\n        \n    var result = [];\n    voxelMap.forEach(function(key, value) {\n        if (value.length > 1)\n            result.push(value);\n    });\n    return result;\n}\n\n","./cdjs/simulator.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\n// Copyright (C) 2015 Apple Inc. All rights reserved.\n// \n// Redistribution and use in source and binary forms, with or without\n// modification, are permitted provided that the following conditions are met:\n//  * Redistributions of source code must retain the above copyright\n//    notice, this list of conditions and the following disclaimer.\n//  * Redistributions in binary form must reproduce the above copyright\n//    notice, this list of conditions and the following disclaimer in the\n//    documentation and/or other materials provided with the distribution.\n//  * Neither the name of the Purdue University nor the\n//    names of its contributors may be used to endorse or promote products\n//    derived from this software without specific prior written permission.\n// \n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\n\nfunction Simulator(numAircraft) {\n    this._aircraft = [];\n    for (var i = 0; i < numAircraft; ++i)\n        this._aircraft.push(new CallSign(\"foo\" + i));\n}\n\nSimulator.prototype.simulate = function(time) {\n    var frame = [];\n    for (var i = 0; i < this._aircraft.length; i += 2) {\n        frame.push({\n            callsign: this._aircraft[i],\n            position: new Vector3D(time, Math.cos(time) * 2 + i * 3, 10)\n        });\n        frame.push({\n            callsign: this._aircraft[i + 1],\n            position: new Vector3D(time, Math.sin(time) * 2 + i * 3, 10)\n        });\n    }\n    return frame;\n};\n\n","./cdjs/util.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\n// Copyright (C) 2015 Apple Inc. All rights reserved.\n// \n// Redistribution and use in source and binary forms, with or without\n// modification, are permitted provided that the following conditions are met:\n//  * Redistributions of source code must retain the above copyright\n//    notice, this list of conditions and the following disclaimer.\n//  * Redistributions in binary form must reproduce the above copyright\n//    notice, this list of conditions and the following disclaimer in the\n//    documentation and/or other materials provided with the distribution.\n//  * Neither the name of the Purdue University nor the\n//    names of its contributors may be used to endorse or promote products\n//    derived from this software without specific prior written permission.\n// \n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\n\nfunction compareNumbers(a, b) {\n    if (a == b)\n        return 0;\n    if (a < b)\n        return -1;\n    if (a > b)\n        return 1;\n    \n    // We say that NaN is smaller than non-NaN.\n    if (a == a)\n        return 1;\n    return -1;\n}\n\nfunction averageAbovePercentile(numbers, percentile) {\n    // Don't change the original array.\n    numbers = numbers.slice();\n    \n    // Sort in ascending order.\n    numbers.sort(function(a, b) { return a - b; });\n    \n    // Now the elements we want are at the end. Keep removing them until the array size shrinks too much.\n    // Examples assuming percentile = 99:\n    //\n    // - numbers.length starts at 100: we will remove just the worst entry and then not remove anymore,\n    //   since then numbers.length / originalLength = 0.99.\n    //\n    // - numbers.length starts at 1000: we will remove the ten worst.\n    //\n    // - numbers.length starts at 10: we will remove just the worst.\n    var numbersWeWant = [];\n    var originalLength = numbers.length;\n    while (numbers.length / originalLength > percentile / 100)\n        numbersWeWant.push(numbers.pop());\n    \n    var sum = 0;\n    for (var i = 0; i < numbersWeWant.length; ++i)\n        sum += numbersWeWant[i];\n    \n    var result = sum / numbersWeWant.length;\n    \n    // Do a sanity check.\n    if (numbers.length && result < numbers[numbers.length - 1]) {\n        throw \"Sanity check fail: the worst case result is \" + result +\n            \" but we didn't take into account \" + numbers;\n    }\n    \n    return result;\n}\n\n","./cdjs/vector_2d.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\n// Copyright (C) 2015 Apple Inc. All rights reserved.\n// \n// Redistribution and use in source and binary forms, with or without\n// modification, are permitted provided that the following conditions are met:\n//  * Redistributions of source code must retain the above copyright\n//    notice, this list of conditions and the following disclaimer.\n//  * Redistributions in binary form must reproduce the above copyright\n//    notice, this list of conditions and the following disclaimer in the\n//    documentation and/or other materials provided with the distribution.\n//  * Neither the name of the Purdue University nor the\n//    names of its contributors may be used to endorse or promote products\n//    derived from this software without specific prior written permission.\n// \n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\n\nfunction Vector2D(x, y) {\n    this.x = x;\n    this.y = y;\n}\n\nVector2D.prototype.plus = function(other) {\n    return new Vector2D(this.x + other.x,\n                        this.y + other.y);\n};\n\nVector2D.prototype.minus = function(other) {\n    return new Vector2D(this.x - other.x,\n                        this.y - other.y);\n};\n\nVector2D.prototype.toString = function() {\n    return \"[\" + this.x + \", \" + this.y + \"]\";\n};\n\nVector2D.prototype.compareTo = function(other) {\n    var result = compareNumbers(this.x, other.x);\n    if (result)\n        return result;\n    return compareNumbers(this.y, other.y);\n};\n\n","./cdjs/vector_3d.js":"// Copyright (c) 2001-2010, Purdue University. All rights reserved.\n// Copyright (C) 2015 Apple Inc. All rights reserved.\n// \n// Redistribution and use in source and binary forms, with or without\n// modification, are permitted provided that the following conditions are met:\n//  * Redistributions of source code must retain the above copyright\n//    notice, this list of conditions and the following disclaimer.\n//  * Redistributions in binary form must reproduce the above copyright\n//    notice, this list of conditions and the following disclaimer in the\n//    documentation and/or other materials provided with the distribution.\n//  * Neither the name of the Purdue University nor the\n//    names of its contributors may be used to endorse or promote products\n//    derived from this software without specific prior written permission.\n// \n// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\n// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\n// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\n// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY\n// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\n// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\n// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\n// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\n// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\n// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\n\nfunction Vector3D(x, y, z) {\n    this.x = x;\n    this.y = y;\n    this.z = z;\n}\n\nVector3D.prototype.plus = function(other) {\n    return new Vector3D(this.x + other.x,\n                        this.y + other.y,\n                        this.z + other.z);\n};\n\nVector3D.prototype.minus = function(other) {\n    return new Vector3D(this.x - other.x,\n                        this.y - other.y,\n                        this.z - other.z);\n};\n\nVector3D.prototype.dot = function(other) {\n    return this.x * other.x + this.y * other.y + this.z * other.z;\n};\n\nVector3D.prototype.squaredMagnitude = function() {\n    return this.dot(this);\n};\n\nVector3D.prototype.magnitude = function() {\n    return Math.sqrt(this.squaredMagnitude());\n};\n\nVector3D.prototype.times = function(amount) {\n    return new Vector3D(this.x * amount,\n                        this.y * amount,\n                        this.z * amount);\n};\n\nVector3D.prototype.as2D = function() {\n    return new Vector2D(this.x, this.y);\n};\n\nVector3D.prototype.toString = function() {\n    return \"[\" + this.x + \", \" + this.y + \", \" + this.z + \"]\";\n};\n\n\n"};
var readFile = function (name) {
    const normalized = String(name).replaceAll("\\", "/");
    if (!Object.prototype.hasOwnProperty.call(__jetstreamResources, normalized))
        throw new Error("JetStream resource not embedded: " + normalized);
    return __jetstreamResources[normalized];
};
var read = function (name, mode) {
    const text = readFile(name);
    if (mode !== "binary")
        return text;
    const bytes = [];
    for (let i = 0; i < text.length; i++)
        bytes.push(text.charCodeAt(i) & 0xff);
    return bytes;
};

"use strict";

/*
 * Copyright (C) 2018-2024 Apple Inc. All rights reserved.
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

const measureTotalTimeAsSubtest = false; // Once we move to preloading all resources, it would be good to turn this on.

const defaultIterationCount = 120;
const defaultWorstCaseCount = 4;

if (!JetStreamParams.prefetchResources && isInBrowser) {
    console.warn("Disabling resource prefetching! All compressed files must have been decompressed using `npm run decompress`");
}

if (JetStreamParams.forceGC && typeof globalThis.gc === "undefined") {
    console.warn("Force-gc is set, but globalThis.gc() is not available.");
}

if (!isInBrowser && JetStreamParams.prefetchResources) {
    // Use the wasm compiled zlib as a polyfill when decompression stream is
    // not available in JS shells.
    load("./wasm/zlib/shell.js");

    // Load a polyfill for TextEncoder/TextDecoder in shells. Used when
    // decompressing a prefetched resource and converting it to text.
    load("./utils/polyfills/fast-text-encoding/1.0.3/text.js");
}

// Used for the promise representing the current benchmark run.
var currentResolve = null;
var currentReject = null;

function displayCategoryScores() {
    document.body.classList.add("details");
}


if (isInBrowser) {
    document.onkeydown = (keyboardEvent) => {
        const key = keyboardEvent.key;
        if (key === "d" || key === "D")
            displayCategoryScores();
    };
}

function sum(values) {
    console.assert(values instanceof Array);
    let sum = 0;
    for (let x of values)
        sum += x;
    return sum;
}

function mean(values) {
    const totalSum = sum(values)
    return totalSum / values.length;
}

function geomeanScore(values) {
    console.assert(values instanceof Array);
    let product = 1;
    for (let x of values)
        product *= x;
    const score = product ** (1 / values.length);
    // Allow 0 for uninitialized subScores().
    console.assert(score >= 0, `Got invalid score: ${score}`)
    return score;
}

function toScore(timeValue) {
    return 5000 / Math.max(timeValue, 1);
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
    return num.toFixed(2);
}

function uiFriendlyScore(num) {
    return uiFriendlyNumber(num);
}

function uiFriendlyDuration(time) {
    return `${time.toFixed(2)} ms`;
}

const LABEL_PADDING = 45;
function shellFriendlyLabel(label) {
    return `${label}`.padEnd(LABEL_PADDING);
}

const VALUE_PADDING = 11;
function shellFriendlyDuration(time) {
    return `${uiFriendlyDuration(time)} `.padStart(VALUE_PADDING);
}

function shellFriendlyScore(time) {
    return `${uiFriendlyScore(time)} pts`.padStart(VALUE_PADDING);
}


// Files can be zlib compressed to reduce the size of the JetStream source code.
// We don't use http compression because we support running from the shell and
// don't want to require a complicated server setup.
//
// zlib was chosen because we already have it in tree for the wasm-zlib test.
function isCompressed(name) {
    return name.endsWith(".z");
}

function uncompressedName(name) {
    console.assert(isCompressed(name));
    return name.slice(0, -2);
}

// TODO: Cleanup / remove / merge. This is only used for caching loads in the
// non-browser setting. In the browser we use exclusively `loadCache`,
// `loadBlob`, `doLoadBlob`, `prefetchResourcesForBrowser` etc., see below.
class ShellFileLoader {
    constructor() {
        this.requests = new Map;
    }

    // Cache / memoize previously read files, because some workloads
    // share common code.
    load(url) {
        console.assert(!isInBrowser);

        let compressed = isCompressed(url);
        if (compressed && !JetStreamParams.prefetchResources) {
            url = uncompressedName(url);
        }

        // If we aren't supposed to prefetch this then return code snippet that will load the url on-demand.
        if (!JetStreamParams.prefetchResources)
            return readFile(url);

        if (this.requests.has(url)) {
            return this.requests.get(url);
        }

        let contents;
        if (compressed) {
            const compressedBytes = new Int8Array(read(url, "binary"));
            const decompressedBytes = zlib.decompress(compressedBytes);
            contents = new TextDecoder().decode(decompressedBytes);
        } else {
            contents = readFile(url);
        }
        this.requests.set(url, contents);
        return contents;
    }
};


class BrowserFileLoader {

    constructor() {
        // TODO: Cleanup / remove / merge `blobDataCache` and `loadCache` vs.
        // the global `fileLoader` cache.
        this.blobDataCache = { __proto__ : null };
        this.loadCache = { __proto__ : null };
    }

    async doLoadBlob(resource) {
        const blobData = this.blobDataCache[resource];

        const compressed = isCompressed(resource);
        if (compressed && !JetStreamParams.prefetchResources) {
            resource = uncompressedName(resource);
        }

        // If we aren't supposed to prefetch this then set the blobURL to just
        // be the resource URL.
        if (!JetStreamParams.prefetchResources) {
            blobData.blobURL = resource;
            return blobData;
        }

        let response;
        let tries = 3;
        while (tries--) {
            let hasError = false;
            try {
                response = await fetch(resource, { cache: "no-store" });
            } catch (e) {
                hasError = true;
            }
            if (!hasError && response.ok)
                break;
            if (tries)
                continue;
            throw new Error("Fetch failed");
        }

        // If we need to decompress this, then run it through a decompression
        // stream.
        if (compressed) {
            const stream = response.body.pipeThrough(new DecompressionStream("deflate"))
            response = new Response(stream);
        }

        let blob = await response.blob();
        blobData.blob = blob;
        blobData.blobURL = URL.createObjectURL(blob);
        return blobData;
    }

    async loadBlob(type, prop, resource, incrementRefCount = true) {
        let blobData = this.blobDataCache[resource];
        if (!blobData) {
            blobData = {
                type: type,
                prop: prop,
                resource: resource,
                blob: null,
                blobURL: null,
                refCount: 0
            };
            this.blobDataCache[resource] = blobData;
        }

        if (incrementRefCount)
            blobData.refCount++;

        let promise = this.loadCache[resource];
        if (promise)
            return promise;

        promise = this.doLoadBlob(resource);
        this.loadCache[resource] = promise;
        return promise;
    }

    async retryPrefetchResource(type, prop, file) {
        console.assert(isInBrowser);

        const counter = JetStream.counter;
        const blobData = this.blobDataCache[file];
        if (blobData.blob) {
            // The same preload blob may be used by multiple subtests. Though the blob is already loaded,
            // we still need to check if this subtest failed to load it before. If so, handle accordingly.
            if (type == "preload") {
                if (this.failedPreloads && this.failedPreloads[blobData.prop]) {
                    this.failedPreloads[blobData.prop] = false;
                    this._preloadBlobData.push({ name: blobData.prop, resource: blobData.resource, blobURLOrPath: blobData.blobURL });
                    counter.failedPreloadResources--;
                }
            }
            return !counter.failedPreloadResources && counter.loadedResources == counter.totalResources;
        }

        // Retry fetching the resource.
        this.loadCache[file] = null;
        await this.loadBlob(type, prop, file, false).then((blobData) => {
            if (!globalThis.allIsGood)
                return;
            if (blobData.type == "preload")
                this._preloadBlobData.push({ name: blobData.prop, resource: blobData.resource, blobURLOrPath: blobData.blobURL });
            this.updateCounter();
        });

        if (!blobData.blob) {
            globalThis.allIsGood = false;
            throw new Error("Fetch failed");
        }

        return !counter.failedPreloadResources && counter.loadedResources == counter.totalResources;
    }

    free(files) {
        for (const file of files) {
            const blobData = this.blobDataCache[file];
            // If we didn't prefetch this resource, then no need to free it
            if (!blobData.blob) {
                continue
            }
            blobData.refCount--;
            if (!blobData.refCount)
                this.blobDataCache[file] = undefined;
        }
    }
}

const browserFileLoader = new BrowserFileLoader();
const shellFileLoader = new ShellFileLoader();

class Driver {
    constructor(benchmarks) {
        this.isReady = false;
        this.isDone = false;
        this.errors = [];
        // Make benchmark list unique and sort it.
        this.benchmarks = Array.from(new Set(benchmarks));
        this.benchmarks.sort((a, b) => a.name.toLowerCase() < b.name.toLowerCase() ? 1 : -1);
        console.assert(this.benchmarks.length, "No benchmarks selected");
        this.counter = { };
        this.counter.loadedResources = 0;
        this.counter.totalResources = 0;
        this.counter.failedPreloadResources = 0;
    }

    async start() {
        let statusElement = false;
        if (isInBrowser) {
            statusElement = document.getElementById("status");
            statusElement.innerHTML = `<label>Running...</label>`;
        } else if (!JetStreamParams.dumpJSONResults)
            console.log("Starting JetStream3");

        performance.mark("update-ui-start");
        const start = performance.now();
        for (const benchmark of this.benchmarks) {
            performance.mark("update-ui-start");
            benchmark.updateUIBeforeRun();
            await updateUI();
            performance.measure("runner update-ui", "update-ui-start");

            try {
                await benchmark.run();
            } catch(e) {
                this.reportError(benchmark, e);
                throw e;
            }

            performance.mark("update-ui-start");
            benchmark.updateUIAfterRun();
            performance.measure("runner update-ui", "update-ui-start");

            if (isInBrowser) {
                browserFileLoader.free(benchmark.files);
            }
        }

        const totalTime = performance.now() - start;
        if (measureTotalTimeAsSubtest) {
            if (isInBrowser)
                document.getElementById("benchmark-total-time-score").innerHTML = uiFriendlyNumber(totalTime);
            else if (!JetStreamParams.dumpJSONResults)
                console.log("Total-Time:", uiFriendlyNumber(totalTime));
            allScores.push(totalTime);
        }

        const allScores = [];
        for (const benchmark of this.benchmarks) {
            const score = benchmark.score;
            console.assert(score > 0, `Invalid ${benchmark.name} score: ${score}`);
            allScores.push(score);
        }

        const categoryScores = new Map();
        const categoryTimes = new Map();
        for (const benchmark of this.benchmarks) {
            for (let category of Object.keys(benchmark.subScores()))
                categoryScores.set(category, []);
            for (let category of Object.keys(benchmark.subTimes()))
                categoryTimes.set(category, []);
        }

        for (const benchmark of this.benchmarks) {
            for (let [category, value] of Object.entries(benchmark.subScores())) {
                const arr = categoryScores.get(category);
                console.assert(value > 0, `Invalid ${benchmark.name} ${category} score: ${value}`);
                arr.push(value);
            }
            for (let [category, value] of Object.entries(benchmark.subTimes())) {
                const arr = categoryTimes.get(category);
                console.assert(value > 0, `Invalid ${benchmark.name} ${category} time: ${value}`);
                arr.push(value);
            }
        }

        const overallScore = geomeanScore(allScores);
        console.assert(overallScore > 0, `Invalid total score: ${overallScore}`);

        if (isInBrowser) {
            let summaryHtml = `<div class="score">${uiFriendlyScore(overallScore)}</div>
                    <label>Score</label>`;
            summaryHtml += `<div class="benchmark benchmark-done">`;
            for (let [category, scores] of categoryScores) {
                summaryHtml += `<span class="result detail">
                                    <span>${uiFriendlyScore(geomeanScore(scores))}</span>
                                    <label>${category}</label>
                                </span>`;
            }
            summaryHtml += "<br/>";
            for (let [category, times] of categoryTimes) {
                summaryHtml += `<span class="result detail">
                                    <span>${uiFriendlyDuration(geomeanScore(times))}</span>
                                    <label>${category}</label>
                                </span>`;
            }
            summaryHtml += "</div>";
            const summaryElement = document.getElementById("result-summary");
            summaryElement.classList.add("done");
            summaryElement.innerHTML = summaryHtml;
            summaryElement.onclick = displayCategoryScores;
            statusElement.innerHTML = "";
        } else if (!JetStreamParams.dumpJSONResults) {
            console.log("Overall:");
            for (let [category, scores] of categoryScores) {
                console.log(
                    shellFriendlyLabel(`Overall ${category}-Score`),
                    shellFriendlyScore(geomeanScore(scores)));
            }
            for (let [category, times] of categoryTimes) {
                console.log(
                    shellFriendlyLabel(`Overall ${category}-Time`),
                    shellFriendlyDuration(geomeanScore(times)));
            }
            console.log("");
            console.log(shellFriendlyLabel("Overall Score"), shellFriendlyScore(overallScore));
            console.log(shellFriendlyLabel("Overall Wall-Time"), shellFriendlyDuration(totalTime));
            console.log("");
        }

        this.reportScoreToRunBenchmarkRunner();
        this.dumpJSONResultsIfNeeded();
        this.isDone = true;

        if (isInBrowser) {
            globalThis.dispatchEvent(new CustomEvent("JetStreamDone", {
                detail: this.resultsObject()
            }));
        }
    }

    prepareBrowserUI() {
        let text = "";
        for (const benchmark of this.benchmarks)
            text += benchmark.renderHTML();

        const resultsTable = document.getElementById("results");
        resultsTable.innerHTML = text;

        document.getElementById("magic").textContent = "";
        document.addEventListener('keypress', (e) => {
            if (e.key === "Enter")
                JetStream.start();
        });
    }

    reportError(benchmark, error) {
        this.pushError(benchmark.name, error);

        if (!isInBrowser)
            return;

        for (const id of benchmark.allScoreIdentifiers())
            document.getElementById(id).innerHTML = "error";
        for (const id of benchmark.allTimeIdentifiers())
            document.getElementById(id).innerHTML = "error";
        const benchmarkResultsUI = document.getElementById(`benchmark-${benchmark.name}`);
        benchmarkResultsUI.classList.remove("benchmark-running");
        benchmarkResultsUI.classList.add("benchmark-error");
    }

    pushError(name, error) {
        this.errors.push({
            benchmark: name,
            error: error.toString(),
            stack: error.stack
        });
    }

    async initialize() {
        if (isInBrowser)
            window.addEventListener("error", (e) => this.pushError("driver startup", e.error));
        await this.prefetchResources();
        this.benchmarks.sort((a, b) => a.name.toLowerCase() < b.name.toLowerCase() ? 1 : -1);
        if (isInBrowser)
            this.prepareBrowserUI();
        this.isReady = true;
        if (isInBrowser) {
            globalThis.dispatchEvent(new Event("JetStreamReady"));
            if (typeof(JetStreamParams.startDelay) !== "undefined") {
                setTimeout(() => this.start(), JetStreamParams.startDelay);
            }
        }
    }

    async prefetchResources() {
        if (!isInBrowser) {
            if (JetStreamParams.prefetchResources) {
                await zlib.initialize();
            }
            for (const benchmark of this.benchmarks)
                benchmark.prefetchResourcesForShell();
            return;
        }

        // TODO: Cleanup the browser path of the preloading below and in
        // `prefetchResourcesForBrowser` / `retryPrefetchResourcesForBrowser`.
        const counter = JetStream.counter;
        const promises = [];
        for (const benchmark of this.benchmarks)
            promises.push(benchmark.prefetchResourcesForBrowser(counter));
        await Promise.all(promises);

        if (counter.failedPreloadResources || counter.loadedResources != counter.totalResources) {
            for (const benchmark of this.benchmarks) {
                const allFilesLoaded = await benchmark.retryPrefetchResourcesForBrowser(counter);
                if (allFilesLoaded)
                    break;
            }

            if (counter.failedPreloadResources || counter.loadedResources != counter.totalResources) {
                // If we've failed to prefetch resources even after a sequential 1 by 1 retry,
                // then fail out early rather than letting subtests fail with a hang.
                globalThis.allIsGood = false;
                throw new Error("Fetch failed");
            }
        }

        JetStream.loadCache = { }; // Done preloading all the files.

        const statusElement = document.getElementById("status");
        statusElement.classList.remove('loading');
        statusElement.innerHTML = `<a href="javascript:JetStream.start()" class="button">Start Test</a>`;
        statusElement.onclick = () => {
            statusElement.onclick = null;
            JetStream.start();
            return false;
        }
    }

    updateCounterUI() {
        const counter = JetStream.counter;
        const statusElement = document.getElementById("status-text");
        statusElement.innerText = `Loading ${counter.loadedResources} of ${counter.totalResources} ...`;

        const percent = (counter.loadedResources / counter.totalResources) * 100;
        const progressBar = document.getElementById("status-progress-bar");
        progressBar.style.width = `${percent}%`;
    }

    resultsObject(format = "run-benchmark") {
        switch(format) {
            case "run-benchmark":
                return this.runBenchmarkResultsObject();
            case "simple":
                return this.simpleResultsObject();
            default:
                throw Error(`Unknown result format '${format}'`);
        }
    }

    runBenchmarkResultsObject()
    {
        let results = {};
        for (const benchmark of this.benchmarks) {
            const subResults = {}
            const subScores = benchmark.subScores();
            for (const name in subScores) {
                subResults[name] = {"metrics": {"Time": {"current": [toTimeValue(subScores[name])]}}};
            }
            results[benchmark.name] = {
                "metrics" : {
                    "Score" : {"current" : [benchmark.score]},
                    "Time": ["Geometric"],
                },
                "tests": subResults,
            };
        }

        results = {"JetStream3.0": {"metrics" : {"Score" : ["Geometric"]}, "tests" : results}};
        return results;
    }

    simpleResultsObject() {
        const results = {__proto__: null};
        for (const benchmark of this.benchmarks) {
            if (!benchmark.isDone)
                continue;
            if (!benchmark.isSuccess) {
                results[benchmark.name] = "FAILED";
            } else {
                results[benchmark.name] = {
                    Score: benchmark.score,
                    ...benchmark.subScores(),

                };
            }
        }
        return results;
    }

    resultsJSON(format = "run-benchmark")
    {
        return JSON.stringify(this.resultsObject(format));
    }

    dumpJSONResultsIfNeeded()
    {
        if (JetStreamParams.dumpJSONResults) {
            console.log("\n");
            console.log(this.resultsJSON());
            console.log("\n");
        }
    }

    dumpTestList()
    {
        for (const benchmark of this.benchmarks) {
            console.log(benchmark.name);
        }
    }

    async reportScoreToRunBenchmarkRunner()
    {
        if (!isInBrowser)
            return;

        if (!JetStreamParams.report)
            return;

        const content = this.resultsJSON();
        await fetch("/report", {
            method: "POST",
            headers: {
                "Content-Type": "application/json",
                "Content-Length": content.length,
                "Connection": "close",
            },
            body: content,
        });
    }
};

const BenchmarkState = Object.freeze({
    READY: "READY",
    SETUP: "SETUP",
    RUNNING: "RUNNING",
    FINALIZE: "FINALIZE",
    ERROR: "ERROR",
    DONE: "DONE"
})


class Scripts {
    constructor(preloads) {
        this.scripts = [];

        let preloadsCode = "";
        let resourcesCode = "";
        for (let { name, resource, blobURLOrPath } of preloads) {
            console.assert(name?.length > 0, "Invalid preload name.");
            console.assert(resource?.length > 0, "Invalid preload resource.");
            console.assert(blobURLOrPath?.length > 0, "Invalid preload data.");
            preloadsCode += `${JSON.stringify(name)}: "${blobURLOrPath}",\n`;
            resourcesCode += `${JSON.stringify(resource)}: "${blobURLOrPath}",\n`;
        }
        // Expose a globalThis.JetStream object to the workload. We use
        // a proxy to prevent prototype access and throw on unknown properties.
        this.add(`
            const throwOnAccess = (name) => new Proxy({},  {
                get(target, property, receiver) {
                    throw new Error(name + "." + property + " is not defined.");
                }
            });
            globalThis.JetStream = {
                __proto__: throwOnAccess("JetStream"),
                preload: {
                    __proto__: throwOnAccess("JetStream.preload"),
                    ${preloadsCode}
                },
                resources: {
                    __proto__: throwOnAccess("JetStream.preload"),
                    ${resourcesCode}
                },
            };
            `);
        this.add(`
            performance.mark ??= function(name) { return { name }};
            performance.measure ??= function() {};
            performance.timeOrigin ??= performance.now();
        `);
    }


    run() {
        throw new Error("Subclasses need to implement this");
    }

    add(text) {
        throw new Error("Subclasses need to implement this");
    }

    addWithURL(url) {
        throw new Error("addWithURL not supported");
    }

    addBrowserTest() {
        this.add(`
            globalThis.JetStream.isInBrowser = ${isInBrowser};
            globalThis.JetStream.isD8 = ${isD8};
        `);
    }

    addDeterministicRandom() {
        this.add(`(() => {
            const initialSeed = 49734321;
            let seed = initialSeed;

            Math.random = () => {
                // Robert Jenkins' 32 bit integer hash function.
                seed = ((seed + 0x7ed55d16) + (seed << 12))  & 0xffff_ffff;
                seed = ((seed ^ 0xc761c23c) ^ (seed >>> 19)) & 0xffff_ffff;
                seed = ((seed + 0x165667b1) + (seed << 5))   & 0xffff_ffff;
                seed = ((seed + 0xd3a2646c) ^ (seed << 9))   & 0xffff_ffff;
                seed = ((seed + 0xfd7046c5) + (seed << 3))   & 0xffff_ffff;
                seed = ((seed ^ 0xb55a4f09) ^ (seed >>> 16)) & 0xffff_ffff;
                // Note that Math.random should return a value that is
                // greater than or equal to 0 and less than 1. Here, we
                // cast to uint32 first then divided by 2^32 for double.
                return (seed >>> 0) / 0x1_0000_0000;
            };

            Math.random.__resetSeed = () => {
                seed = initialSeed;
            };
        })();`);
    }
}

class ShellScripts extends Scripts {
    constructor(preloads) {
        super(preloads);
        this.prefetchedResources = Object.create(null);;
    }

    run() {
        let globalObject;
        let realm;
        if (isD8) {
            realm = Realm.createAllowCrossRealmAccess();
            globalObject = Realm.global(realm);
            globalObject.loadString = function(s) {
                return Realm.eval(realm, s);
            };
            globalObject.readFile = read;
        } else if (isSpiderMonkey) {
            globalObject = newGlobal();
            globalObject.loadString = globalObject.evaluate;
            globalObject.readFile = globalObject.readRelativeToScript;
        } else
            globalObject = runString("");

        // Expose console copy in the realm so we don't accidentally modify
        // the original object.
        globalObject.console = Object.assign({}, console);
        globalObject.self = globalObject;
        globalObject.top = {
            currentResolve,
            currentReject
        };

        // Pass the prefetched resources to the benchmark global.
        if (JetStreamParams.prefetchResources) {
            // Pass the 'TextDecoder' polyfill into the benchmark global. Don't
            // use 'TextDecoder' as that will get picked up in the kotlin test
            // without full support.
            globalObject.ShellTextDecoder = TextDecoder;
            // Store shellPrefetchedResources on ShellPrefetchedResources so that
            // getBinary and getString can find them.
            globalObject.ShellPrefetchedResources = this.prefetchedResources;
        } else {
            console.assert(Object.values(this.prefetchedResources).length === 0, "Unexpected prefetched resources");
        }

        globalObject.performance ??= performance;
        globalObject.loadString(this.scripts.join("\n"));

        return isD8 ? realm : globalObject;
    }

    addPrefetchedResources(prefetchedResources) {
        for (let [file, bytes] of Object.entries(prefetchedResources)) {
            this.prefetchedResources[file] = bytes;
        }
    }

    add(text) {
        this.scripts.push(text);
    }

    addWithURL(url) {
        console.assert(false, "Should not reach here in CLI");
    }
}

class BrowserScripts extends Scripts {
    constructor(preloads) {
        super(preloads);
        this.add("window.onerror = top.currentReject;");
    }

    run() {
        const string = this.scripts.join("\n");
        const magic = document.getElementById("magic");
        magic.contentDocument.body.textContent = "";
        magic.contentDocument.body.innerHTML = `<iframe id="magicframe" frameborder="0">`;

        const magicFrame = magic.contentDocument.getElementById("magicframe");
        magicFrame.contentDocument.open();
        magicFrame.contentDocument.write(`<!DOCTYPE html>
            <head>
               <title>benchmark payload</title>
            </head>
            <body>${string}</body>
        </html>`);
        return magicFrame;
    }

    add(text) {
        this.scripts.push(`<script>${text}</script>`);
    }

    addWithURL(url) {
        this.scripts.push(`<script src="${url}"></script>`);
    }
}


class Benchmark {
    constructor({
            name,
            files,
            preload = {},
            tags,
            iterations,
            deterministicRandom = false,
            exposeBrowserTest = false,
            allowUtf16 = false,
            args = {} }) {
        this._state = BenchmarkState.READY;
        this.results = [];

        this.name = name
        this.tags = this._processTags(tags)
        this._arguments = args;

        this.iterations = this._processIterationCount(iterations);
        this._deterministicRandom = deterministicRandom;
        this._exposeBrowserTest = exposeBrowserTest;
        this.allowUtf16 = !!allowUtf16;

        // Resource handling:
        this._scripts = null;
        this._files = files;
        this._preloadEntries = Object.entries(preload);
        this._preloadBlobData = [];
        this._shellPrefetchedResources = null;
    }

    // Use getter so it can be overridden in subclasses (GroupedBenchmark).
    get files() {
        return this._files;
    }
    get preloadEntries() {
        return this._preloadEntries;
    }

    _processTags(rawTags) {
        const tags = new Set(rawTags.map(each => each.toLowerCase()));
        if (tags.size != rawTags.length)
            throw new Error(`${this.name} got duplicate tags: ${rawTags.join()}`);
        tags.add("all");
        if (!tags.has("default"))
            tags.add("disabled");
        return tags;
    }

    _processIterationCount(iterations) {
        if (this.name in JetStreamParams.testIterationCountMap)
            return JetStreamParams.testIterationCountMap[this.name];
        if (JetStreamParams.testIterationCount)
            return JetStreamParams.testIterationCount;
        if (iterations)
            return iterations;
        return defaultIterationCount;
    }

    _processWorstCaseCount(worstCaseCount) {
        if (this.name in JetStreamParams.testWorstCaseCountMap)
            return JetStreamParams.testWorstCaseCountMap[this.name];
        if (JetStreamParams.testWorstCaseCount !== undefined)
            return JetStreamParams.testWorstCaseCount;
        if (worstCaseCount !== undefined)
            return worstCaseCount;
        return defaultWorstCaseCount;
    }

    get isDone() {
        return this._state == BenchmarkState.DONE || this._state == BenchmarkState.ERROR;
    }
    get isSuccess() { return this._state = BenchmarkState.DONE; }

    hasAnyTag(...tags) {
        return tags.some((tag) => this.tags.has(tag.toLowerCase()));
    }

    get benchmarkArguments() {
        return {
            ...this._arguments,
            iterationCount: this.iterations,
        };
    }

    get runnerCode() {
        return `{
            const benchmark = new Benchmark(${JSON.stringify(this.benchmarkArguments)});
            const results = [];
            const benchmarkName = "${this.name}";

            for (let i = 0; i < ${this.iterations}; i++) {
                ${this.preIterationCode}

                const iterationMarkLabel = benchmarkName + "-iteration-" + i;
                const iterationStartMark = performance.mark(iterationMarkLabel);

                const start = performance.now();
                benchmark.runIteration(i);
                const end = performance.now();

                performance.measure(iterationMarkLabel, iterationMarkLabel);

                ${this.postIterationCode}

                results.push(Math.max(1, end - start));
            }
            benchmark.validate?.(${this.iterations});
            top.currentResolve(results);
        };`;
    }

    processResults(results) {
        this.results = Array.from(results);
        return this.results;
    }

    get score() {
        const subScores = Object.values(this.subScores());
        return geomeanScore(subScores);
    }

    get totalTime() {
        const subTimes = Object.values(this.subTimes());
        return sum(subTimes);
    }

    get wallTime() {
        return this.endTime - this.startTime;
    }

    subScores() {
        throw new Error("Subclasses need to implement this");
    }

    subTimes() {
        throw new Error("Subclasses need to implement this");
    }

    allScores() {
        const allScores = this.subScores();
        allScores["Score"] = this.score;
        return allScores;
    }

    allTimes() {
        const allTimes = this.subTimes();
        allTimes["Total"] = this.totalTime;
        allTimes["Wall"] = this.wallTime;
        return allTimes;
    }

    get prerunCode() { return null; }


    get preIterationCode() {
        let code = this.prepareForNextIterationCode ;
        if (this._deterministicRandom)
            code += `Math.random.__resetSeed();`;

        if (JetStreamParams.customPreIterationCode)
            code += JetStreamParams.customPreIterationCode;

        return code;
    }

    get prepareForNextIterationCode() {
        return "benchmark.prepareForNextIteration?.();"
    }

    get postIterationCode() {
        let code = "";

        if (JetStreamParams.customPostIterationCode)
            code += JetStreamParams.customPostIterationCode;

        return code;
    }

    renderHTML() {
        const scoreDescription = Object.keys(this.allScores());
        const timeDescription = Object.keys(this.allTimes());

        const scoreIds = this.allScoreIdentifiers();
        const overallScoreId = scoreIds.pop();
        const timeIds = this.allTimeIdentifiers();
        let text = `<div class="benchmark" id="benchmark-${this.name}">
            <h3 class="benchmark-name">${this.name} <a class="info" href="in-depth.html#${this.name}">i</a></h3>
            <h4 class="score" id="${overallScoreId}">&nbsp;</h4>
            <h4 class="plot" id="plot-${this.name}">&nbsp;</h4>
            <p>`;
        for (let i = 0; i < scoreIds.length; i++) {
            const scoreId = scoreIds[i];
            const label = scoreDescription[i];
            text += `<span class="result"><span id="${scoreId}">&nbsp;</span><label>${label}</label></span>`;
        }
        text += "<br/>";
        for (let i = 0; i < timeIds.length; i++) {
            const timeId = timeIds[i];
            const label = timeDescription[i];
            text += `<span class="result detail"><span id="${timeId}">&nbsp;</span><label>${label}</label></span>`;
        }
        text += `</p></div>`;
        return text;
    }

    async run() {
        if (this.isDone)
            throw new Error(`Cannot run Benchmark ${this.name} twice`);
        this._state = BenchmarkState.PREPARE;

        if (JetStreamParams.forceGC) {
            // This will trigger for individual benchmarks in
            // GroupedBenchmarks since they delegate .run() to their inner
            // non-grouped benchmarks.
            globalThis?.gc();
        }

        const scripts = isInBrowser ?
                new BrowserScripts(this._preloadBlobData) :
                new ShellScripts(this._preloadBlobData);

        if (this._deterministicRandom)
            scripts.addDeterministicRandom()
        if (this._exposeBrowserTest)
            scripts.addBrowserTest();

        if (this._shellPrefetchedResources) {
            scripts.addPrefetchedResources(this._shellPrefetchedResources);
        }

        const prerunCode = this.prerunCode;
        if (prerunCode)
            scripts.add(prerunCode);

        if (!isInBrowser) {
            console.assert(this._scripts && this._scripts.length === this.files.length);
            for (const text of this._scripts)
                scripts.add(text);
        } else {
            const cache = browserFileLoader.blobDataCache;
            for (const file of this.files) {
                scripts.addWithURL(cache[file].blobURL);
            }
        }

        const promise = new Promise((resolve, reject) => {
            currentResolve = resolve;
            currentReject = reject;
        });

        scripts.add(this.runnerCode);

        performance.mark(this.name);
        this.startTime = performance.now();

        if (JetStreamParams.RAMification)
            resetMemoryPeak();

        let magicFrame;
        try {
            this._state = BenchmarkState.RUNNING;
            magicFrame = scripts.run();
        } catch(e) {
            this._state = BenchmarkState.ERROR;
            console.log("Error in runCode: ", e);
            console.log(e.stack);
            throw e;
        } finally {
            this._state = BenchmarkState.FINALIZE;
        }
        const results = await promise;

        this.endTime = performance.now();
        performance.measure(this.name, this.name);

        if (JetStreamParams.RAMification) {
            const memoryFootprint = MemoryFootprint();
            this.currentFootprint = memoryFootprint.current;
            this.peakFootprint = memoryFootprint.peak;
        }

        this.processResults(results);
        this._state = BenchmarkState.DONE;

        if (isInBrowser)
            magicFrame.contentDocument.close();
        else if (isD8)
            Realm.dispose(magicFrame);
    }


    updateCounter() {
        const counter = JetStream.counter;
        ++counter.loadedResources;
        JetStream.updateCounterUI();
    }

    prefetchResourcesForBrowser(counter) {
        console.assert(isInBrowser);

        const promises = this.files.map((file) => browserFileLoader.loadBlob("file", null, file).then((blobData) => {
                if (!globalThis.allIsGood)
                    return;
                this.updateCounter();
            }).catch((error) => {
                // We'll try again later in retryPrefetchResourceForBrowser(). Don't throw an error.
            }));

        for (const [name, resource] of this.preloadEntries) {
            promises.push(browserFileLoader.loadBlob("preload", name, resource).then((blobData) => {
                if (!globalThis.allIsGood)
                    return;
                this._preloadBlobData.push({ name: blobData.prop, resource: blobData.resource, blobURLOrPath: blobData.blobURL });
                this.updateCounter();
            }).catch((error) => {
                // We'll try again later in retryPrefetchResourceForBrowser(). Don't throw an error.
                if (!this.failedPreloads)
                    this.failedPreloads = { };
                this.failedPreloads[name] = true;
                counter.failedPreloadResources++;
            }));
        }

        JetStream.counter.totalResources += promises.length;
        return Promise.all(promises);
    }

    async retryPrefetchResourcesForBrowser(counter) {
        // FIXME: Move to BrowserFileLoader.
        console.assert(isInBrowser);

        for (const resource of this.files) {
            const allDone = await browserFileLoader.retryPrefetchResource("file", null, resource);

            if (allDone)
                return true; // All resources loaded, nothing more to do.
        }

        for (const [name, resource] of this.preloadEntries) {
            const allDone = await browserFileLoader.retryPrefetchResource("preload", name, resource);
            if (allDone)
                return true; // All resources loaded, nothing more to do.
        }
        return !counter.failedPreloadResources && counter.loadedResources == counter.totalResources;
    }

    prefetchResourcesForShell() {
        // FIXME: move to ShellFileLoader.
        console.assert(!isInBrowser);

        console.assert(this._scripts === null, "This initialization should be called only once.");
        this._scripts = this.files.map(file => shellFileLoader.load(file));

        console.assert(this._preloadBlobData.length === 0, "This initialization should be called only once.");
        this._shellPrefetchedResources = Object.create(null);
        for (let [name, resource] of this.preloadEntries) {
            const compressed = isCompressed(resource);
            if (compressed && !JetStreamParams.prefetchResources) {
                resource = uncompressedName(resource);
            }

            if (JetStreamParams.prefetchResources) {
                let bytes = new Int8Array(read(resource, "binary"));
                if (compressed) {
                    bytes = zlib.decompress(bytes);
                }
                this._shellPrefetchedResources[resource] = bytes;
            }

            this._preloadBlobData.push({ name, resource, blobURLOrPath: resource });
        }
    }

    allScoreIdentifiers() {
        const ids = Object.keys(this.allScores()).map(name => this.scoreIdentifier(name));
        return ids;
    }

    scoreIdentifier(scoreName) {
        return `results-cell-${this.name}-${scoreName}`;
    }

    allTimeIdentifiers() {
        const ids = Object.keys(this.allTimes()).map(name => this.timeIdentifier(name));
        return ids;
    }

    timeIdentifier(scoreName) {
        return `results-cell-${this.name}-${scoreName}-time`;
    }

    updateUIBeforeRun() {
        if (!JetStreamParams.dumpJSONResults)
            this.updateConsoleBeforeRun();
        if (isInBrowser)
            this.updateUIBeforeRunInBrowser();
    }

    updateConsoleBeforeRun() {
        console.log(`Running ${this.name}:`);
    }

    updateUIBeforeRunInBrowser() {
        const resultsBenchmarkUI = document.getElementById(`benchmark-${this.name}`);
        resultsBenchmarkUI.classList.add("benchmark-running");
        resultsBenchmarkUI.scrollIntoView({ block: "nearest" });

        for (const id of this.allScoreIdentifiers())
            document.getElementById(id).innerHTML = "...";
        for (const id of this.allTimeIdentifiers())
            document.getElementById(id).innerHTML = "...";
    }

    updateUIAfterRun() {
        if (isInBrowser)
            this.updateUIAfterRunInBrowser();
        if (JetStreamParams.dumpJSONResults)
            return;
        this.updateConsoleAfterRun();
    }

    updateUIAfterRunInBrowser() {
        const benchmarkResultsUI = document.getElementById(`benchmark-${this.name}`);
        benchmarkResultsUI.classList.remove("benchmark-running");
        benchmarkResultsUI.classList.add("benchmark-done");

        for (const [name, value] of Object.entries(this.allScores()))
            document.getElementById(this.scoreIdentifier(name)).innerHTML = uiFriendlyScore(value);
        for (const [name, value] of Object.entries(this.allTimes()))
            document.getElementById(this.timeIdentifier(name)).innerHTML = uiFriendlyDuration(value);

        this.renderScatterPlot();
    }

    updateConsoleAfterRun() {
        for (let [name, value] of Object.entries(this.allScores())) {
            if (!name.endsWith("Score"))
                name = `${name}-Score`;

            this.logMetric(name, shellFriendlyScore(value));
        }
        for (let [name, value] of Object.entries(this.allTimes())) {
            this.logMetric(`${name}-Time`, shellFriendlyDuration(value));
        }
        if (JetStreamParams.RAMification) {
            this.logMetric("Current Footprint", uiFriendlyNumber(this.currentFootprint));
            this.logMetric("Peak Footprint", uiFriendlyNumber(this.peakFootprint));
        }
        console.log("");
    }

    logMetric(name, value) {
        console.log(
            shellFriendlyLabel(`${this.name} ${name}`),
            value);
    }

    renderScatterPlot() {
        const plotContainer = document.getElementById(`plot-${this.name}`);
        if (!plotContainer || !this.results || this.results.length === 0)
            return;

        const scores = this.results.map(time => toScore(time));
        const scoreElement = document.getElementById(this.scoreIdentifier("Score"));
        const width = scoreElement.offsetWidth;
        const height = scoreElement.offsetHeight;

        const padding = 5;
        const maxResult = Math.max(...scores);
        const minResult = Math.min(...scores);

        const xRatio = (width - 2 * padding) / (scores.length - 1 || 1);
        const yRatio = (height - 2 * padding) / (maxResult - minResult || 1);
        const radius = Math.max(1.5, Math.min(2.5, 10 - (this.iterations / 10)));

        let circlesSVG = "";
        for (let i = 0; i < scores.length; i++) {
            const result = scores[i];
            const cx = padding + i * xRatio;
            const cy = height - padding - (result - minResult) * yRatio;
            const title = `Iteration ${i + 1}: ${uiFriendlyScore(result)} (${uiFriendlyDuration(this.results[i])})`;
            circlesSVG += `<circle cx="${cx}" cy="${cy}" r="${radius}"><title>${title}</title></circle>`;
        }
        plotContainer.innerHTML = `<svg width="${width}px" height="${height}px">${circlesSVG}</svg>`;
    }
};

class GroupedBenchmark extends Benchmark {
    constructor(plan, benchmarks) {
        super(plan);
        console.assert(benchmarks.length);
        for (const benchmark of benchmarks) {
            // FIXME: Tags don't work for grouped tests anyway but if they did then this would be weird and probably wrong.
            console.assert(!benchmark.hasAnyTag("Default"), `Grouped benchmark sub-benchmarks shouldn't have the "Default" tag`, benchmark.tags);
        }
        benchmarks.sort((a, b) => a.name.toLowerCase() < b.name.toLowerCase() ? 1 : -1);
        this.benchmarks = benchmarks;
    }

    async prefetchResourcesForBrowser(counter) {
        for (const benchmark of this.benchmarks)
            await benchmark.prefetchResourcesForBrowser(counter);
    }

    async retryPrefetchResourcesForBrowser(counter) {
        for (const benchmark of this.benchmarks)
            await benchmark.retryPrefetchResourcesForBrowser(counter);
    }

    prefetchResourcesForShell() {
        for (const benchmark of this.benchmarks)
            benchmark.prefetchResourcesForShell();
    }

    renderHTML() {
        let text = super.renderHTML();
        if (JetStreamParams.groupDetails) {
            for (const benchmark of this.benchmarks)
                text += benchmark.renderHTML();
        }
        return text;
    }

    updateConsoleBeforeRun() {
        if (!JetStreamParams.groupDetails)
            super.updateConsoleBeforeRun();
    }

    updateConsoleAfterRun(scoreEntries) {
        if (JetStreamParams.groupDetails)
            super.updateConsoleBeforeRun();
        super.updateConsoleAfterRun(scoreEntries);
    }

    get files() {
        return this.benchmarks.flatMap(benchmark => benchmark.files)
    }

    get preloadEntries() {
        return this.benchmarks.flatMap(benchmark => benchmark.preloadEntries)
    }

    async run() {
        this._state = BenchmarkState.PREPARE;
        performance.mark(this.name);
        this.startTime = performance.now();

        let benchmark;
        try {
            this._state = BenchmarkState.RUNNING;
            for (benchmark of this.benchmarks) {
                if (JetStreamParams.groupDetails)
                    benchmark.updateUIBeforeRun();
                await benchmark.run();
                if (JetStreamParams.groupDetails)
                    benchmark.updateUIAfterRun();
            }
        } catch (e) {
            this._state = BenchmarkState.ERROR;
            console.log(`Error in runCode of grouped benchmark ${benchmark.name}: `, e);
            console.log(e.stack);
            throw e;
        } finally {
            this._state = BenchmarkState.FINALIZE;
        }

        this.endTime = performance.now();
        performance.measure(this.name, this.name);

        this.processResults();
        this._state = BenchmarkState.DONE;
    }

    processResults() {
        this.results = [];
        for (const benchmark of this.benchmarks)
            this.results = this.results.concat(benchmark.results);
    }

    subScores() {
        const results = {};

        for (const benchmark of this.benchmarks) {
            let scores = benchmark.subScores();
            for (let subScore in scores) {
                results[subScore] ??= [];
                results[subScore].push(scores[subScore]);
            }
        }

        for (let subScore in results)
            results[subScore] = geomeanScore(results[subScore]);
        return results;
    }

    subTimes() {
        const results = {};

        for (const benchmark of this.benchmarks) {
            let times = benchmark.subTimes();
            for (let subTime in times) {
                results[subTime] ??= [];
                results[subTime].push(times[subTime]);
            }
        }

        for (let subTimes in results)
            results[subTimes] = sum(results[subTimes]);
        return results;
    }
};

class DefaultBenchmark extends Benchmark {
    constructor({worstCaseCount, ...args}) {
        super(args);

        this.worstCaseCount = this._processWorstCaseCount(worstCaseCount);
        this.firstIterationTime = null;
        this.firstIterationScore = null;
        this.worstTime = null;
        this.worstScore = null;
        this.averageTime = null;
        this.averageScore = null;
        if (this.worstCaseCount)
            console.assert(this.iterations > this.worstCaseCount);
        console.assert(this.worstCaseCount >= 0);
    }

    processResults(results) {
        results = super.processResults(results)

        this.firstIterationTime = results[0];
        this.firstIterationScore = toScore(results[0]);

        results = results.slice(1);
        results.sort((a, b) => a < b ? 1 : -1);
        for (let i = 0; i + 1 < results.length; ++i)
            console.assert(results[i] >= results[i + 1]);

        if (this.worstCaseCount) {
            const worstCase = [];
            for (let i = 0; i < this.worstCaseCount; ++i)
                worstCase.push(results[i]);
            this.worstTime = mean(worstCase);
            this.worstScore = toScore(this.worstTime);
        }
        this.averageTime = mean(results);
        this.averageScore = toScore(this.averageTime);
    }

    subScores() {
        const scores = { "First": this.firstIterationScore }
        if (this.worstCaseCount)
            scores["Worst"] = this.worstScore;
        if (this.iterations > 1)
            scores["Average"] = this.averageScore;
        return scores;
    }

    subTimes() {
        const times = {
            "First": this.firstIterationTime,
        };
        if (this.worstCaseCount)
            times["Worst"] = this.worstTime;
        if (this.iterations > 1)
            times["Average"] = this.averageTime;
        return times;
    }
}

class AsyncBenchmark extends DefaultBenchmark {
    get prerunCode() {
        let str = "";
        // FIXME: It would be nice if these were available to any benchmark not just async ones but since these functions
        // are async they would only work in a context where the benchmark is async anyway. Long term, we should do away
        // with this class and make all benchmarks async.
        if (isInBrowser) {
            str += `
                JetStream.getBinary = async function(blobURL) {
                    const response = await fetch(blobURL);
                    return new Int8Array(await response.arrayBuffer());
                };

                JetStream.getString = async function(blobURL) {
                    const response = await fetch(blobURL);
                    return response.text();
                };

                JetStream.dynamicImport = async function(blobURL) {
                    return await import(blobURL);
                };
            `;
        } else {
            str += `
                JetStream.getBinary = async function(path) {
                    if ("ShellPrefetchedResources" in globalThis) {
                        return ShellPrefetchedResources[path];
                    }
                    return new Int8Array(read(path, "binary"));
                };

                JetStream.getString = async function(path) {
                    if ("ShellPrefetchedResources" in globalThis) {
                        return new ShellTextDecoder().decode(ShellPrefetchedResources[path]);
                    }
                    return read(path);
                };

                JetStream.dynamicImport = async function(path) {
                    try {
                        // TODO: this skips the prefetched resources, but I'm
                        // not sure of a way around that.
                        return await import(path);
                    } catch (e) {
                        // In shells, relative imports require different paths, so try with and
                        // without the "./" prefix (e.g., JSC requires it).
                        return await import(path.slice("./".length))
                    }
                };
            `;
        }
        return str;
    }

    get prepareForNextIterationCode() {
        return "await benchmark.prepareForNextIteration?.();"
    }

    get runnerCode() {
        return `
        async function doRun() {
            const benchmark = new Benchmark(${JSON.stringify(this.benchmarkArguments)});
            await benchmark.init?.();
            const results = [];
            const benchmarkName = "${this.name}";

            for (let i = 0; i < ${this.iterations}; i++) {
                ${this.preIterationCode}

                const iterationMarkLabel = benchmarkName + "-iteration-" + i;
                const iterationStartMark = performance.mark(iterationMarkLabel);

                const start = performance.now();
                await benchmark.runIteration(i);
                const end = performance.now();

                performance.measure(iterationMarkLabel, iterationMarkLabel);

                ${this.postIterationCode}

                results.push(Math.max(1, end - start));
            }
            benchmark.validate?.(${this.iterations});
            top.currentResolve(results);
        };
        doRun().catch((error) => { top.currentReject(error); });`
    }
};

// Meant for wasm benchmarks that are directly compiled with an emcc build script. It might not work for benchmarks built as
// part of a larger project's build system or a wasm benchmark compiled from a language that doesn't compile with emcc.
class WasmEMCCBenchmark extends AsyncBenchmark {
    get prerunCode() {
        let str = `
            let verbose = false;

            let globalObject = this;

            abort = quit = function() {
                if (verbose)
                    console.log('Intercepted quit/abort');
            };

            const oldPrint = globalObject.print;
            globalObject.print = globalObject.printErr = (...args) => {
                if (verbose)
                    console.log('Intercepted print: ', ...args);
            };

            let Module = {
                preRun: [],
                postRun: [],
                noInitialRun: true,
                print: print,
                printErr: printErr
            };

            globalObject.Module = Module;
            ${super.prerunCode};
        `;

        return str;
    }
};

class WSLBenchmark extends Benchmark {
    constructor(plan) {
        super(plan);

        this.stdlibTime = null;
        this.stdlibScore = null;
        this.mainRunTime = null;
        this.mainRunScore = null;
    }

    processResults(results) {
        results = super.processResults(results);
        this.stdlibTime = results[0];
        this.stdlibScore = toScore(results[0]);
        this.mainRunTime = results[1];
        this.mainRunScore = toScore(results[1]);
    }

    get runnerCode() {
        return `{
            const benchmark = new Benchmark(${JSON.stringify(this.benchmarkArguments)});
            const benchmarkName = "${this.name}";

            const results = [];
            {
                const markLabel = benchmarkName + "-stdlib";
                const startMark = performance.mark(markLabel);

                const start = performance.now();
                benchmark.buildStdlib();
                results.push(performance.now() - start);

                performance.measure(markLabel, markLabel);
            }

            {
                const markLabel = benchmarkName + "-mainRun";
                const startMark = performance.mark(markLabel);

                const start = performance.now();
                benchmark.run();
                results.push(performance.now() - start);

                performance.measure(markLabel, markLabel);
            }
            top.currentResolve(results);
        }`;
    }

    subTimes() {
        return {
            "Stdlib": this.stdlibTime,
            "MainRun": this.mainRunTime,
        };
    }

    subScores() {
        return {
            "Stdlib": this.stdlibScore,
            "MainRun": this.mainRunScore,
        };
    }
};

class AsyncWasmLegacyBenchmark extends Benchmark {
    constructor(plan) {
        super(plan);
        this.startupTime = null;
        this.startupScore = null;
        this.runTime = null;
        this.runScore = null;
    }

    processResults(results) {
        results = super.processResults(results);
        this.startupTime = results[0];
        this.startupScore= toScore(results[0]);
        this.runTime = results[1];
        this.runScore = toScore(results[1]);
    }

    get prerunCode() {
        const str = `
            let verbose = false;

            let compileTime = null;
            let runTime = null;

            let globalObject = this;

            globalObject.benchmarkTime = performance.now.bind(performance);

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

            const oldConsoleLog = globalObject.console.log;
            globalObject.print = globalObject.printErr = (...args) => {
                if (verbose)
                    oldConsoleLog('Intercepted print: ', ...args);
            };

            let Module = {
                preRun: [],
                postRun: [],
                print: globalObject.print,
                printErr: globalObject.print
            };
            globalObject.Module = Module;
        `;
        return str;
    }

    get runnerCode() {
        let str = `JetStream.loadBlob = function(key, path, andThen) {`;

        if (isInBrowser) {
            str += `
                const xhr = new XMLHttpRequest();
                xhr.open('GET', path, true);
                xhr.responseType = 'arraybuffer';
                xhr.onload = function() {
                    Module[key] = new Int8Array(xhr.response);
                    andThen();
                };
                xhr.send(null);
            `;
        } else {
            str += `
            if (ShellPrefetchedResources) {
                Module[key] = ShellPrefetchedResources[path];
            } else {
                Module[key] = new Int8Array(read(path, "binary"));
            }
            if (andThen == doRun) {
                globalObject.read = (...args) => {
                    console.log("should not be inside read: ", ...args);
                    throw new Error;
                };
            };

            Promise.resolve(42).then(() => {
                try {
                    andThen();
                } catch(e) {
                    console.log("error running wasm:", e);
                    console.log(e.stack);
                    throw e;
                }
            });
            `;
        }

        str += "};\n";
        let preloadCount = 0;
        for (const [name, resource] of this.preloadEntries) {
            preloadCount++;
            str += `JetStream.loadBlob(${JSON.stringify(name)}, "${resource}", () => {\n`;
        }
        str += `doRun().catch((e) => {
            console.log("error running wasm:", e);
            console.log(e.stack)
            throw e;
        });`;
        for (let i = 0; i < preloadCount; ++i) {
            str += `})`;
        }
        str += `;`;

        return str;
    }

    subScores() {
        return {
            "Startup": this.startupScore,
            "Runtime": this.runScore,
        };
    }

    subTimes() {
        return {
            "Startup": this.startupTime,
            "Runtime": this.runTime,
        };
    }
};

function dotnetPreloads(type)
{
    return {
        dotnetUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/dotnet.js`,
        dotnetNativeUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/dotnet.native.js`,
        dotnetRuntimeUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/dotnet.runtime.js`,
        wasmBinaryUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/dotnet.native.wasm`,
        icuCustomUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/icudt_CJK.dat`,
        dllCollectionsConcurrentUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Collections.Concurrent.wasm`,
        dllCollectionsUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Collections.wasm`,
        dllComponentModelPrimitivesUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.ComponentModel.Primitives.wasm`,
        dllComponentModelTypeConverterUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.ComponentModel.TypeConverter.wasm`,
        dllDrawingPrimitivesUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Drawing.Primitives.wasm`,
        dllDrawingUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Drawing.wasm`,
        dllIOPipelinesUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.IO.Pipelines.wasm`,
        dllLinqUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Linq.wasm`,
        dllMemoryUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Memory.wasm`,
        dllObjectModelUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.ObjectModel.wasm`,
        dllPrivateCorelibUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Private.CoreLib.wasm`,
        dllRuntimeInteropServicesJavaScriptUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Runtime.InteropServices.JavaScript.wasm`,
        dllTextEncodingsWebUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Text.Encodings.Web.wasm`,
        dllTextJsonUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Text.Json.wasm`,
        dllAppUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/dotnet.wasm`,
    }
}

let BENCHMARKS = [
    // ARES
    new DefaultBenchmark({
        name: "Air",
        files: [
            "./ARES-6/Air/symbols.js",
            "./ARES-6/Air/tmp_base.js",
            "./ARES-6/Air/arg.js",
            "./ARES-6/Air/basic_block.js",
            "./ARES-6/Air/code.js",
            "./ARES-6/Air/frequented_block.js",
            "./ARES-6/Air/inst.js",
            "./ARES-6/Air/opcode.js",
            "./ARES-6/Air/reg.js",
            "./ARES-6/Air/stack_slot.js",
            "./ARES-6/Air/tmp.js",
            "./ARES-6/Air/util.js",
            "./ARES-6/Air/custom.js",
            "./ARES-6/Air/liveness.js",
            "./ARES-6/Air/insertion_set.js",
            "./ARES-6/Air/allocate_stack.js",
            "./ARES-6/Air/payload-gbemu-executeIteration.js",
            "./ARES-6/Air/payload-imaging-gaussian-blur-gaussianBlur.js",
            "./ARES-6/Air/payload-airjs-ACLj8C.js",
            "./ARES-6/Air/payload-typescript-scanIdentifier.js",
            "./ARES-6/Air/benchmark.js",
        ],
        tags: ["default", "js", "ARES"],
    }),
    new DefaultBenchmark({
        name: "Basic",
        files: [
            "./ARES-6/Basic/ast.js",
            "./ARES-6/Basic/basic.js",
            "./ARES-6/Basic/caseless_map.js",
            "./ARES-6/Basic/lexer.js",
            "./ARES-6/Basic/number.js",
            "./ARES-6/Basic/parser.js",
            "./ARES-6/Basic/random.js",
            "./ARES-6/Basic/state.js",
            "./ARES-6/Basic/benchmark.js",
        ],
        tags: ["default", "js",  "ARES"],
    }),
    new DefaultBenchmark({
        name: "ML",
        files: [
            "./ARES-6/ml/index.js",
            "./ARES-6/ml/benchmark.js",
        ],
        iterations: 60,
        tags: ["default", "js",  "ARES"],
    }),
    new AsyncBenchmark({
        name: "Babylon",
        files: [
            "./ARES-6/Babylon/index.js",
            "./ARES-6/Babylon/benchmark.js",
        ],
        preload: {
            airBlob: "./ARES-6/Babylon/air-blob.js",
            basicBlob: "./ARES-6/Babylon/basic-blob.js",
            inspectorBlob: "./ARES-6/Babylon/inspector-blob.js",
            babylonBlob: "./ARES-6/Babylon/babylon-blob.js",
        },
        tags: ["default", "js",  "ARES"],
        allowUtf16: true,
    }),
    // CDJS
    new DefaultBenchmark({
        name: "cdjs",
        files: [
            "./cdjs/constants.js",
            "./cdjs/util.js",
            "./cdjs/red_black_tree.js",
            "./cdjs/call_sign.js",
            "./cdjs/vector_2d.js",
            "./cdjs/vector_3d.js",
            "./cdjs/motion.js",
            "./cdjs/reduce_collision_set.js",
            "./cdjs/simulator.js",
            "./cdjs/collision.js",
            "./cdjs/collision_detector.js",
            "./cdjs/benchmark.js",
        ],
        iterations: 60,
        worstCaseCount: 3,
        tags: ["default", "js",  ],
    }),
    // CodeLoad
    new AsyncBenchmark({
        name: "first-inspector-code-load",
        files: [
            "./code-load/code-first-load.js",
        ],
        preload: {
            inspectorPayloadBlob: "./code-load/inspector-payload-minified.js",
        },
        tags: ["default", "js", "inspector", "codeload"],
    }),
    new AsyncBenchmark({
        name: "multi-inspector-code-load",
        files: [
            "./code-load/code-multi-load.js",
        ],
        preload: {
            inspectorPayloadBlob: "./code-load/inspector-payload-minified.js",
        },
        tags: ["default", "js", "inspector", "codeload"],
    }),
    // Octane
    new DefaultBenchmark({
        name: "Box2D",
        files: [
            "./Octane/box2d.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "octane-code-load",
        files: [
            "./Octane/code-first-load.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js", "codeload", "Octane"],
    }),
    new DefaultBenchmark({
        name: "crypto",
        files: [
            "./Octane/crypto.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "delta-blue",
        files: [
            "./Octane/deltablue.js"
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "earley-boyer",
        files: [
            "./Octane/earley-boyer.js"
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "gbemu",
        files: [
            "./Octane/gbemu-part1.js",
            "./Octane/gbemu-part2.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "mandreel",
        files: [
            "./Octane/mandreel.js"
        ],
        iterations: 80,
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "navier-stokes",
        files: [
            "./Octane/navier-stokes.js",
        ],
        deterministicRandom: true,
        tags: ["default",  "js", "Octane"],
    }),
    new DefaultBenchmark({
        name: "pdfjs",
        files: [
            "./Octane/pdfjs.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "raytrace",
        files: [
            "./Octane/raytrace.js",
        ],
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "regexp-octane",
        files: [
            "./Octane/regexp.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js", "regexp", "Octane"],
    }),
    new DefaultBenchmark({
        name: "richards",
        files: [
            "./Octane/richards.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "splay",
        files: [
            "./Octane/splay.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "typescript-octane",
        files: [
            "./Octane/typescript-compiler.js",
            "./Octane/typescript-input.js",
            "./Octane/typescript.js",
        ],
        iterations: 15,
        worstCaseCount: 2,
        deterministicRandom: true,
        tags: ["Octane", "js",  "typescript"],
    }),
    // RexBench
    new DefaultBenchmark({
        name: "FlightPlanner",
        files: [
            "./RexBench/FlightPlanner/airways.js",
            "./RexBench/FlightPlanner/waypoints.js.z",
            "./RexBench/FlightPlanner/flight_planner.js",
            "./RexBench/FlightPlanner/expectations.js",
            "./RexBench/FlightPlanner/benchmark.js",
        ],
        tags: ["default", "js",  "RexBench"],
    }),
    new DefaultBenchmark({
        name: "OfflineAssembler",
        files: [
            "./RexBench/OfflineAssembler/registers.js",
            "./RexBench/OfflineAssembler/instructions.js",
            "./RexBench/OfflineAssembler/ast.js",
            "./RexBench/OfflineAssembler/parser.js",
            "./RexBench/OfflineAssembler/file.js",
            "./RexBench/OfflineAssembler/LowLevelInterpreter.js",
            "./RexBench/OfflineAssembler/LowLevelInterpreter32_64.js",
            "./RexBench/OfflineAssembler/LowLevelInterpreter64.js",
            "./RexBench/OfflineAssembler/InitBytecodes.js",
            "./RexBench/OfflineAssembler/expected.js",
            "./RexBench/OfflineAssembler/benchmark.js",
        ],
        iterations: 80,
        tags: ["default", "js",  "RexBench"],
    }),
    new DefaultBenchmark({
        name: "UniPoker",
        files: [
            "./RexBench/UniPoker/poker.js",
            "./RexBench/UniPoker/expected.js",
            "./RexBench/UniPoker/benchmark.js",
        ],
        deterministicRandom: true,
        // FIXME: UniPoker should not access isInBrowser.
        exposeBrowserTest: true,
        tags: ["default", "js",  "RexBench"],
    }),
    new DefaultBenchmark({
        name: "validatorjs",
        files: [
            // Use the unminified version for easier local profiling.
            // "./validatorjs/dist/bundle.es6.js",
            "./validatorjs/dist/bundle.es6.min.js",
            "./validatorjs/benchmark.js",
        ],
        tags: ["default", "js",  "regexp"],
    }),
    // Simple
    new DefaultBenchmark({
        name: "hash-map",
        files: [
            "./simple/hash-map.js",
        ],
        tags: ["default", "js",  "Simple"],
    }),
    new AsyncBenchmark({
        name: "doxbee-promise",
        files: [
            "./simple/doxbee-promise.js",
        ],
        tags: ["default",  "js", "promise", "Simple"],
    }),
    new AsyncBenchmark({
        name: "doxbee-async",
        files: [
            "./simple/doxbee-async.js",
        ],
        tags: ["default", "js", "Simple"],
    }),
    // SeaMonster
    new DefaultBenchmark({
        name: "ai-astar",
        files: [
            "./SeaMonster/ai-astar.js"
        ],
        tags: ["default", "js", "SeaMonster"],
    }),
    new DefaultBenchmark({
        name: "gaussian-blur",
        files: [
            "./SeaMonster/gaussian-blur.js",
        ],
        tags: ["default", "js", "SeaMonster"],
    }),
    new DefaultBenchmark({
        name: "stanford-crypto-aes",
        files: [
            "./SeaMonster/sjlc.js",
            "./SeaMonster/stanford-crypto-aes.js",
        ],
        tags: ["default", "js", "SeaMonster"],
    }),
    new DefaultBenchmark({
        name: "stanford-crypto-pbkdf2",
        files: [
            "./SeaMonster/sjlc.js",
            "./SeaMonster/stanford-crypto-pbkdf2.js"
        ],
        tags: ["default", "js", "SeaMonster"],
    }),
    new DefaultBenchmark({
        name: "stanford-crypto-sha256",
        files: [
            "./SeaMonster/sjlc.js",
            "./SeaMonster/stanford-crypto-sha256.js",
        ],
        tags: ["default", "js", "SeaMonster"],
    }),
    new DefaultBenchmark({
        name: "json-stringify-inspector",
        files: [
            "./SeaMonster/inspector-json-payload.js.z",
            "./SeaMonster/json-stringify-inspector.js",
        ],
        iterations: 20,
        worstCaseCount: 2,
        tags: ["default", "js", "json", "inspector", "SeaMonster"],
    }),
    new DefaultBenchmark({
        name: "json-parse-inspector",
        files: [
            "./SeaMonster/inspector-json-payload.js.z",
            "./SeaMonster/json-parse-inspector.js",
        ],
        iterations: 20,
        worstCaseCount: 2,
        tags: ["default", "js", "json", "inspector", "SeaMonster"],
    }),
    // BigInt
    new AsyncBenchmark({
        name: "bigint-noble-bls12-381",
        files: [
            "./bigint/web-crypto-sham.js",
            "./bigint/noble-bls12-381-bundle.js",
            "./bigint/noble-benchmark.js",
        ],
        iterations: 4,
        worstCaseCount: 1,
        deterministicRandom: true,
        tags: ["js", "bigint", "BigIntNoble"],
    }),
    new AsyncBenchmark({
        name: "bigint-noble-secp256k1",
        files: [
            "./bigint/web-crypto-sham.js",
            "./bigint/noble-secp256k1-bundle.js",
            "./bigint/noble-benchmark.js",
        ],
        deterministicRandom: true,
        tags: ["js", "bigint", "BigIntNoble"],
    }),
    new AsyncBenchmark({
        name: "bigint-noble-ed25519",
        files: [
            "./bigint/web-crypto-sham.js",
            "./bigint/noble-ed25519-bundle.js",
            "./bigint/noble-benchmark.js",
        ],
        iterations: 30,
        deterministicRandom: true,
        tags: ["default", "js", "bigint", "BigIntNoble"],
    }),
    new DefaultBenchmark({
        name: "bigint-paillier",
        files: [
            "./bigint/web-crypto-sham.js",
            "./bigint/paillier-bundle.js",
            "./bigint/paillier-benchmark.js",
        ],
        iterations: 10,
        worstCaseCount: 2,
        deterministicRandom: true,
        tags: ["js", "bigint", "BigIntMisc"],
    }),
    new DefaultBenchmark({
        name: "bigint-bigdenary",
        files: [
            "./bigint/bigdenary-bundle.js",
            "./bigint/bigdenary-benchmark.js",
        ],
        iterations: 160,
        worstCaseCount: 16,
        tags: ["js", "bigint", "BigIntMisc"],
    }),
    // Proxy
    new AsyncBenchmark({
        name: "proxy-mobx",
        files: [
            "./proxy/common.js",
            "./proxy/mobx-bundle.js",
            "./proxy/mobx-benchmark.js",
        ],
        iterations: defaultIterationCount * 3,
        worstCaseCount: defaultWorstCaseCount * 3,
        tags: ["default", "js", "Proxy"],
    }),
    new AsyncBenchmark({
        name: "proxy-vue",
        files: [
            "./proxy/common.js",
            "./proxy/vue-bundle.js",
            "./proxy/vue-benchmark.js",
        ],
        tags: ["default", "js", "Proxy"],
    }),
    new AsyncBenchmark({
        name: "mobx-startup",
        files: [
            "./utils/StartupBenchmark.js",
            "./mobx/benchmark.js",
        ],
        preload: {
            // Debug Sources for nicer profiling.
            // BUNDLE: "./mobx/dist/bundle.es6.js",
            BUNDLE: "./mobx/dist/bundle.es6.min.js",
        },
        tags: ["default", "js", "mobx", "startup", "es6"],
        iterations: 30,
        worstCaseCount: 3,
    }),
    new AsyncBenchmark({
        name: "jsdom-d3-startup",
        files: [
            "./utils/StartupBenchmark.js",
            "./jsdom-d3-startup/benchmark.js",
        ],
        preload: {
            // Unminified sources for profiling.
            // BUNDLE: "./jsdom-d3-startup/dist/bundle.js",
            BUNDLE: "./jsdom-d3-startup/dist/bundle.min.js",
            US_DATA: "./jsdom-d3-startup/data/counties-albers-10m.json",
            AIRPORTS: "./jsdom-d3-startup/data/airports.csv",
        },
        tags: ["default", "js", "d3", "startup", "jsdom"],
        iterations: 15,
        worstCaseCount: 2,
    }),
    new AsyncBenchmark({
        name: "web-ssr",
        files: [
            "./utils/StartupBenchmark.js",
            "./web-ssr/benchmark.js",
        ],
        preload: {
            // Debug Sources for nicer profiling.
            // BUNDLE: "./web-ssr/dist/bundle.js",
            BUNDLE: "./web-ssr/dist/bundle.min.js",
        },
        tags: ["default", "js", "web", "ssr"],
        iterations: 30,
    }),
    // Class fields
    new DefaultBenchmark({
        name: "raytrace-public-class-fields",
        files: [
            "./class-fields/raytrace-public-class-fields.js",
        ],
        tags: ["default", "js", "ClassFields"],
    }),
    new DefaultBenchmark({
        name: "raytrace-private-class-fields",
        files: [
            "./class-fields/raytrace-private-class-fields.js",
        ],
        tags: ["default", "js", "ClassFields"],
    }),
    new AsyncBenchmark({
        name: "typescript-lib",
        files: [
            "./TypeScript/src/mock/sys.js",
            "./TypeScript/dist/bundle.js",
            "./TypeScript/benchmark.js",
        ],
        preload: {
            // Large test project:
            // "tsconfig": "./TypeScript/src/gen/zod-medium/tsconfig.json",
            // "files": "./TypeScript/src/gen/zod-medium/files.json",
            "tsconfig": "./TypeScript/src/gen/immer-tiny/tsconfig.json",
            "files": "./TypeScript/src/gen/immer-tiny/files.json",
        },
        iterations: 1,
        worstCaseCount: 0,
        tags: ["default", "js", "typescript"],
    }),
    // Generators
    new AsyncBenchmark({
        name: "async-fs",
        files: [
            "./generators/async-file-system.js",
        ],
        iterations: 80,
        worstCaseCount: 6,
        deterministicRandom: true,
        tags: ["default", "js", "Generators"],
    }),
    new DefaultBenchmark({
        name: "sync-fs",
        files: [
            "./generators/sync-file-system.js",
        ],
        iterations: 80,
        worstCaseCount: 6,
        deterministicRandom: true,
        tags: ["default", "js", "Generators"],
    }),
    new DefaultBenchmark({
        name: "lazy-collections",
        files: [
            "./generators/lazy-collections.js",
        ],
        tags: ["default", "js", "Generators"],
    }),
    new DefaultBenchmark({
        name: "js-tokens",
        files: [
            "./generators/js-tokens.js",
        ],
        tags: ["default", "js", "Generators"],
    }),
    new DefaultBenchmark({
        name: "threejs",
        files: [
            "./threejs/three.js",
            "./threejs/benchmark.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js"],
    }),
    // Wasm
    new WasmEMCCBenchmark({
        name: "HashSet-wasm",
        files: [
            "./wasm/HashSet/build/HashSet.js",
            "./wasm/HashSet/benchmark.js",
        ],
        preload: {
            wasmBinary: "./wasm/HashSet/build/HashSet.wasm",
        },
        iterations: 50,
        // No longer run by-default: We have more realistic Wasm workloads by
        // now, and it was over-incentivizing inlining.
        tags: ["Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "quicksort-wasm",
        files: [
            "./wasm/quicksort/build/quicksort.js",
            "./wasm/quicksort/benchmark.js",
        ],
        preload: {
            wasmBinary: "./wasm/quicksort/build/quicksort.wasm",
        },
        iterations: 50,
        // No longer run by-default: We have more realistic Wasm workloads by
        // now, and it was a small microbenchmark.
        tags: ["Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "gcc-loops-wasm",
        files: [
            "./wasm/gcc-loops/build/gcc-loops.js",
            "./wasm/gcc-loops/benchmark.js",
        ],
        preload: {
            wasmBinary: "./wasm/gcc-loops/build/gcc-loops.wasm",
        },
        iterations: 50,
        // No longer run by-default: We have more realistic Wasm workloads by
        // now, and it was a small microbenchmark.
        tags: ["Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "tsf-wasm",
        files: [
            "./wasm/TSF/build/tsf.js",
            "./wasm/TSF/benchmark.js",
        ],
        preload: {
            wasmBinary: "./wasm/TSF/build/tsf.wasm",
        },
        iterations: 50,
        tags: ["default", "Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "richards-wasm",
        files: [
            "./wasm/richards/build/richards.js",
            "./wasm/richards/benchmark.js",
        ],
        preload: {
            wasmBinary: "./wasm/richards/build/richards.wasm",
        },
        iterations: 50,
        tags: ["default", "Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "sqlite3-wasm",
        files: [
            "./utils/polyfills/fast-text-encoding/1.0.3/text.js",
            "./sqlite3/benchmark.js",
            "./sqlite3/build/jswasm/speedtest1.js",
        ],
        preload: {
            wasmBinary: "./sqlite3/build/jswasm/speedtest1.wasm",
        },
        iterations: 30,
        worstCaseCount: 2,
        tags: ["default", "Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "Dart-flute-complex-wasm",
        files: [
            "./Dart/benchmark.js",
        ],
        preload: {
            jsModule: "./Dart/build/flute.complex.dart2wasm.mjs",
            wasmBinary: "./Dart/build/flute.complex.dart2wasm.wasm",
        },
        iterations: 15,
        worstCaseCount: 2,
        // Not run by default because the `CupertinoTimePicker` widget is very allocation-heavy,
        // leading to an unrealistic GC-dominated workload. See
        // https://github.com/WebKit/JetStream/pull/97#issuecomment-3139924169
        // The todomvc workload below is less allocation heavy and a replacement for now.
        // TODO: Revisit, once Dart/Flutter worked on this widget or workload.
        tags: ["Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "Dart-flute-todomvc-wasm",
        files: [
            "./Dart/benchmark.js",
        ],
        preload: {
            jsModule: "./Dart/build/flute.todomvc.dart2wasm.mjs",
            wasmBinary: "./Dart/build/flute.todomvc.dart2wasm.wasm",
        },
        iterations: 30,
        worstCaseCount: 2,
        tags: ["default", "Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "Kotlin-compose-wasm",
        files: [
            "./Kotlin-compose/benchmark.js",
        ],
        preload: {
            skikoJsModule: "./Kotlin-compose/build/skiko.mjs",
            skikoWasmBinary: "./Kotlin-compose/build/skiko.wasm",
            composeJsModule: "./Kotlin-compose/build/compose-benchmarks-benchmarks.uninstantiated.mjs",
            composeWasmBinary: "./Kotlin-compose/build/compose-benchmarks-benchmarks.wasm",
            inputImageCompose: "./Kotlin-compose/build/compose-multiplatform.png",
            inputImageCat: "./Kotlin-compose/build/example1_cat.jpg",
            inputImageComposeCommunity: "./Kotlin-compose/build/example1_compose-community-primary.png",
            inputFontItalic: "./Kotlin-compose/build/jetbrainsmono_italic.ttf",
            inputFontRegular: "./Kotlin-compose/build/jetbrainsmono_regular.ttf"
        },
        iterations: 5,
        worstCaseCount: 1,
        tags: ["default", "Wasm"],
    }),
    new AsyncBenchmark({
        name: "transformersjs-bert-wasm",
        files: [
            "./utils/polyfills/fast-text-encoding/1.0.3/text.js",
            "./transformersjs/benchmark.js",
            "./transformersjs/task-bert.js",
        ],
        preload: {
            transformersJsModule: "./transformersjs/build/transformers.js",

            onnxJsModule: "./transformersjs/build/onnxruntime-web/ort-wasm-simd-threaded.mjs",
            onnxWasmBinary: "./transformersjs/build/onnxruntime-web/ort-wasm-simd-threaded.wasm",

            modelWeights: "./transformersjs/build/models/Xenova/distilbert-base-uncased-finetuned-sst-2-english/onnx/model_uint8.onnx",
            modelConfig: "./transformersjs/build/models/Xenova/distilbert-base-uncased-finetuned-sst-2-english/config.json",
            modelTokenizer: "./transformersjs/build/models/Xenova/distilbert-base-uncased-finetuned-sst-2-english/tokenizer.json",
            modelTokenizerConfig: "./transformersjs/build/models/Xenova/distilbert-base-uncased-finetuned-sst-2-english/tokenizer_config.json",
        },
        iterations: 30,
        allowUtf16: true,
        tags: ["default", "Wasm", "transformersjs"],
    }),
    new AsyncBenchmark({
        name: "transformersjs-whisper-wasm",
        files: [
            "./utils/polyfills/fast-text-encoding/1.0.3/text.js",
            "./transformersjs/benchmark.js",
            "./transformersjs/task-whisper.js",
        ],
        preload: {
            transformersJsModule: "./transformersjs/build/transformers.js",

            onnxJsModule: "./transformersjs/build/onnxruntime-web/ort-wasm-simd-threaded.mjs",
            onnxWasmBinary: "./transformersjs/build/onnxruntime-web/ort-wasm-simd-threaded.wasm",

            modelEncoderWeights: "./transformersjs/build/models/Xenova/whisper-tiny.en/onnx/encoder_model_quantized.onnx",
            modelDecoderWeights: "./transformersjs/build/models/Xenova/whisper-tiny.en/onnx/decoder_model_merged_quantized.onnx",
            modelConfig: "./transformersjs/build/models/Xenova/whisper-tiny.en/config.json",
            modelTokenizer: "./transformersjs/build/models/Xenova/whisper-tiny.en/tokenizer.json",
            modelTokenizerConfig: "./transformersjs/build/models/Xenova/whisper-tiny.en/tokenizer_config.json",
            modelPreprocessorConfig: "./transformersjs/build/models/Xenova/whisper-tiny.en/preprocessor_config.json",
            modelGenerationConfig: "./transformersjs/build/models/Xenova/whisper-tiny.en/generation_config.json",

            inputFile: "./transformersjs/build/inputs/jfk.raw",
        },
        iterations: 5,
        worstCaseCount: 1,
        allowUtf16: true,
        tags: ["Wasm", "transformersjs"],
    }),
    new AsyncWasmLegacyBenchmark({
        name: "tfjs-wasm",
        files: [
            "./wasm/tfjs-model-helpers.js",
            "./wasm/tfjs-model-mobilenet-v3.js",
            "./wasm/tfjs-model-mobilenet-v1.js",
            "./wasm/tfjs-model-coco-ssd.js",
            "./wasm/tfjs-model-use.js",
            "./wasm/tfjs-model-use-vocab.js",
            "./wasm/tfjs-bundle.js",
            "./wasm/tfjs.js",
            "./wasm/tfjs-benchmark.js",
        ],
        preload: {
            tfjsBackendWasmBlob: "./wasm/tfjs-backend-wasm.wasm",
        },
        deterministicRandom: true,
        exposeBrowserTest: true,
        allowUtf16: true,
        tags: ["Wasm"],
    }),
    new AsyncWasmLegacyBenchmark({
        name: "tfjs-wasm-simd",
        files: [
            "./wasm/tfjs-model-helpers.js",
            "./wasm/tfjs-model-mobilenet-v3.js",
            "./wasm/tfjs-model-mobilenet-v1.js",
            "./wasm/tfjs-model-coco-ssd.js",
            "./wasm/tfjs-model-use.js",
            "./wasm/tfjs-model-use-vocab.js",
            "./wasm/tfjs-bundle.js",
            "./wasm/tfjs.js",
            "./wasm/tfjs-benchmark.js",
        ],
        preload: {
            tfjsBackendWasmSimdBlob: "./wasm/tfjs-backend-wasm-simd.wasm",
        },
        deterministicRandom: true,
        exposeBrowserTest: true,
        allowUtf16: true,
        tags: ["Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "argon2-wasm",
        files: [
            "./wasm/argon2/build/argon2.js",
            "./wasm/argon2/benchmark.js",
        ],
        preload: {
            wasmBinary: "./wasm/argon2/build/argon2.wasm.z",
        },
        iterations: 30,
        worstCaseCount: 3,
        deterministicRandom: true,
        allowUtf16: true,
        tags: ["default", "Wasm"],
    }),
    new AsyncBenchmark({
        name: "babylonjs-startup-es5",
        files: [
            "./utils/StartupBenchmark.js",
            "./babylonjs/benchmark/startup.js",
        ],
        preload: {
            BUNDLE: "./babylonjs/dist/bundle.es5.min.js",
        },
        args: {
            expectedCacheCommentCount: 23988,
        },
        tags: ["startup",  "js", "class", "es5", "babylonjs"],
        iterations: 10,
    }),
    new AsyncBenchmark({
        name: "babylonjs-startup-es6",
        files: [
            "./utils/StartupBenchmark.js",
            "./babylonjs/benchmark/startup.js",
        ],
        preload: {
            BUNDLE: "./babylonjs/dist/bundle.es6.min.js",
        },
        args: {
            expectedCacheCommentCount: 21222,
        },
        tags: ["Default",  "js", "startup", "class", "es6", "babylonjs"],
        iterations: 10,
    }),
    new AsyncBenchmark({
        name: "babylonjs-scene-es5",
        files: [
            // Use non-minified sources for easier profiling:
            // "./babylonjs/dist/bundle.es5.js",
            "./babylonjs/dist/bundle.es5.min.js",
            "./babylonjs/benchmark/scene.js",
        ],
        preload: {
            PARTICLES_BLOB: "./babylonjs/data/particles.json",
            PIRATE_FORT_BLOB: "./babylonjs/data/pirateFort.glb",
            CANNON_BLOB: "./babylonjs/data/cannon.glb",
        },
        tags: ["scene", "js",  "es5", "babylonjs"],
        iterations: 5,
    }),
    new AsyncBenchmark({
        name: "babylonjs-scene-es6",
        files: [
            // Use non-minified sources for easier profiling:
            // "./babylonjs/dist/bundle.es6.js",
            "./babylonjs/dist/bundle.es6.min.js",
            "./babylonjs/benchmark/scene.js",
        ],
        preload: {
            PARTICLES_BLOB: "./babylonjs/data/particles.json",
            PIRATE_FORT_BLOB: "./babylonjs/data/pirateFort.glb",
            CANNON_BLOB: "./babylonjs/data/cannon.glb",
        },
        tags: ["Default", "js", "scene", "es6", "babylonjs"],
        iterations: 5,
    }),
    // WorkerTests
    new AsyncBenchmark({
        name: "bomb-workers",
        files: [
            "./worker/bomb.js",
        ],
        exposeBrowserTest: true,
        iterations: 80,
        preload: {
            rayTrace3D: "./worker/bomb-subtests/3d-raytrace.js",
            accessNbody: "./worker/bomb-subtests/access-nbody.js",
            morph3D: "./worker/bomb-subtests/3d-morph.js",
            cube3D: "./worker/bomb-subtests/3d-cube.js",
            accessFunnkuch: "./worker/bomb-subtests/access-fannkuch.js",
            accessBinaryTrees: "./worker/bomb-subtests/access-binary-trees.js",
            accessNsieve: "./worker/bomb-subtests/access-nsieve.js",
            bitopsBitwiseAnd: "./worker/bomb-subtests/bitops-bitwise-and.js",
            bitopsNsieveBits: "./worker/bomb-subtests/bitops-nsieve-bits.js",
            controlflowRecursive: "./worker/bomb-subtests/controlflow-recursive.js",
            bitops3BitBitsInByte: "./worker/bomb-subtests/bitops-3bit-bits-in-byte.js",
            botopsBitsInByte: "./worker/bomb-subtests/bitops-bits-in-byte.js",
            cryptoAES: "./worker/bomb-subtests/crypto-aes.js",
            cryptoMD5: "./worker/bomb-subtests/crypto-md5.js",
            cryptoSHA1: "./worker/bomb-subtests/crypto-sha1.js",
            dateFormatTofte: "./worker/bomb-subtests/date-format-tofte.js",
            dateFormatXparb: "./worker/bomb-subtests/date-format-xparb.js",
            mathCordic: "./worker/bomb-subtests/math-cordic.js",
            mathPartialSums: "./worker/bomb-subtests/math-partial-sums.js",
            mathSpectralNorm: "./worker/bomb-subtests/math-spectral-norm.js",
            stringBase64: "./worker/bomb-subtests/string-base64.js",
            stringFasta: "./worker/bomb-subtests/string-fasta.js",
            stringValidateInput: "./worker/bomb-subtests/string-validate-input.js",
            stringTagcloud: "./worker/bomb-subtests/string-tagcloud.js",
            stringUnpackCode: "./worker/bomb-subtests/string-unpack-code.js",
            regexpDNA: "./worker/bomb-subtests/regexp-dna.js",
        },
        tags: ["default", "js", "WorkerTests"],
    }),
    new AsyncBenchmark({
        name: "segmentation",
        files: [
            "./worker/segmentation.js",
        ],
        preload: {
            asyncTaskBlob: "./worker/async-task.js",
        },
        iterations: 36,
        worstCaseCount: 3,
        tags: ["default", "js",  "WorkerTests"],
    }),
    // WSL
    new WSLBenchmark({
        name: "WSL",
        files: [
            "./WSL/Node.js",
            "./WSL/Type.js",
            "./WSL/ReferenceType.js",
            "./WSL/Value.js",
            "./WSL/Expression.js",
            "./WSL/Rewriter.js",
            "./WSL/Visitor.js",
            "./WSL/CreateLiteral.js",
            "./WSL/CreateLiteralType.js",
            "./WSL/PropertyAccessExpression.js",
            "./WSL/AddressSpace.js",
            "./WSL/AnonymousVariable.js",
            "./WSL/ArrayRefType.js",
            "./WSL/ArrayType.js",
            "./WSL/Assignment.js",
            "./WSL/AutoWrapper.js",
            "./WSL/Block.js",
            "./WSL/BoolLiteral.js",
            "./WSL/Break.js",
            "./WSL/CallExpression.js",
            "./WSL/CallFunction.js",
            "./WSL/Check.js",
            "./WSL/CheckLiteralTypes.js",
            "./WSL/CheckLoops.js",
            "./WSL/CheckRecursiveTypes.js",
            "./WSL/CheckRecursion.js",
            "./WSL/CheckReturns.js",
            "./WSL/CheckUnreachableCode.js",
            "./WSL/CheckWrapped.js",
            "./WSL/Checker.js",
            "./WSL/CloneProgram.js",
            "./WSL/CommaExpression.js",
            "./WSL/ConstexprFolder.js",
            "./WSL/ConstexprTypeParameter.js",
            "./WSL/Continue.js",
            "./WSL/ConvertPtrToArrayRefExpression.js",
            "./WSL/DereferenceExpression.js",
            "./WSL/DoWhileLoop.js",
            "./WSL/DotExpression.js",
            "./WSL/DoubleLiteral.js",
            "./WSL/DoubleLiteralType.js",
            "./WSL/EArrayRef.js",
            "./WSL/EBuffer.js",
            "./WSL/EBufferBuilder.js",
            "./WSL/EPtr.js",
            "./WSL/EnumLiteral.js",
            "./WSL/EnumMember.js",
            "./WSL/EnumType.js",
            "./WSL/EvaluationCommon.js",
            "./WSL/Evaluator.js",
            "./WSL/ExpressionFinder.js",
            "./WSL/ExternalOrigin.js",
            "./WSL/Field.js",
            "./WSL/FindHighZombies.js",
            "./WSL/FlattenProtocolExtends.js",
            "./WSL/FlattenedStructOffsetGatherer.js",
            "./WSL/FloatLiteral.js",
            "./WSL/FloatLiteralType.js",
            "./WSL/FoldConstexprs.js",
            "./WSL/ForLoop.js",
            "./WSL/Func.js",
            "./WSL/FuncDef.js",
            "./WSL/FuncInstantiator.js",
            "./WSL/FuncParameter.js",
            "./WSL/FunctionLikeBlock.js",
            "./WSL/HighZombieFinder.js",
            "./WSL/IdentityExpression.js",
            "./WSL/IfStatement.js",
            "./WSL/IndexExpression.js",
            "./WSL/InferTypesForCall.js",
            "./WSL/Inline.js",
            "./WSL/Inliner.js",
            "./WSL/InstantiateImmediates.js",
            "./WSL/IntLiteral.js",
            "./WSL/IntLiteralType.js",
            "./WSL/Intrinsics.js",
            "./WSL/LateChecker.js",
            "./WSL/Lexer.js",
            "./WSL/LexerToken.js",
            "./WSL/LiteralTypeChecker.js",
            "./WSL/LogicalExpression.js",
            "./WSL/LogicalNot.js",
            "./WSL/LoopChecker.js",
            "./WSL/MakeArrayRefExpression.js",
            "./WSL/MakePtrExpression.js",
            "./WSL/NameContext.js",
            "./WSL/NameFinder.js",
            "./WSL/NameResolver.js",
            "./WSL/NativeFunc.js",
            "./WSL/NativeFuncInstance.js",
            "./WSL/NativeType.js",
            "./WSL/NativeTypeInstance.js",
            "./WSL/NormalUsePropertyResolver.js",
            "./WSL/NullLiteral.js",
            "./WSL/NullType.js",
            "./WSL/OriginKind.js",
            "./WSL/OverloadResolutionFailure.js",
            "./WSL/Parse.js",
            "./WSL/Prepare.js",
            "./WSL/Program.js",
            "./WSL/ProgramWithUnnecessaryThingsRemoved.js",
            "./WSL/PropertyResolver.js",
            "./WSL/Protocol.js",
            "./WSL/ProtocolDecl.js",
            "./WSL/ProtocolFuncDecl.js",
            "./WSL/ProtocolRef.js",
            "./WSL/PtrType.js",
            "./WSL/ReadModifyWriteExpression.js",
            "./WSL/RecursionChecker.js",
            "./WSL/RecursiveTypeChecker.js",
            "./WSL/ResolveNames.js",
            "./WSL/ResolveOverloadImpl.js",
            "./WSL/ResolveProperties.js",
            "./WSL/ResolveTypeDefs.js",
            "./WSL/Return.js",
            "./WSL/ReturnChecker.js",
            "./WSL/ReturnException.js",
            "./WSL/StandardLibrary.js",
            "./WSL/StatementCloner.js",
            "./WSL/StructLayoutBuilder.js",
            "./WSL/StructType.js",
            "./WSL/Substitution.js",
            "./WSL/SwitchCase.js",
            "./WSL/SwitchStatement.js",
            "./WSL/SynthesizeEnumFunctions.js",
            "./WSL/SynthesizeStructAccessors.js",
            "./WSL/TrapStatement.js",
            "./WSL/TypeDef.js",
            "./WSL/TypeDefResolver.js",
            "./WSL/TypeOrVariableRef.js",
            "./WSL/TypeParameterRewriter.js",
            "./WSL/TypeRef.js",
            "./WSL/TypeVariable.js",
            "./WSL/TypeVariableTracker.js",
            "./WSL/TypedValue.js",
            "./WSL/UintLiteral.js",
            "./WSL/UintLiteralType.js",
            "./WSL/UnificationContext.js",
            "./WSL/UnreachableCodeChecker.js",
            "./WSL/VariableDecl.js",
            "./WSL/VariableRef.js",
            "./WSL/VisitingSet.js",
            "./WSL/WSyntaxError.js",
            "./WSL/WTrapError.js",
            "./WSL/WTypeError.js",
            "./WSL/WhileLoop.js",
            "./WSL/WrapChecker.js",
            "./WSL/Test.js",
        ],
        tags: ["default", "js", "WSL"],
    }),
    // 8bitbench
    new WasmEMCCBenchmark({
        name: "8bitbench-wasm",
        files: [
            "./utils/polyfills/fast-text-encoding/1.0.3/text.js",
            "./8bitbench/build/rust/pkg/emu_bench.js",
            "./8bitbench/benchmark.js",
        ],
        preload: {
            wasmBinary: "./8bitbench/build/rust/pkg/emu_bench_bg.wasm",
            romBinary: "./8bitbench/build/assets/program.bin",
        },
        iterations: 15,
        worstCaseCount: 2,
        tags: ["default", "Wasm"],
    }),
    // zlib-wasm
    new WasmEMCCBenchmark({
        name: "zlib-wasm",
        files: [
            "./wasm/zlib/build/zlib.js",
            "./wasm/zlib/benchmark.js",
        ],
        preload: {
            wasmBinary: "./wasm/zlib/build/zlib.wasm",
        },
        iterations: 40,
        tags: ["default", "Wasm"],
    }),
    // .NET
    new AsyncBenchmark({
        name: "dotnet-interp-wasm",
        files: [
            "./wasm/dotnet/interp.js",
            "./wasm/dotnet/benchmark.js",
        ],
        preload: dotnetPreloads("interp"),
        iterations: 10,
        worstCaseCount: 2,
        tags: ["default", "Wasm", "dotnet"],
    }),
    new AsyncBenchmark({
        name: "dotnet-aot-wasm",
        files: [
            "./wasm/dotnet/aot.js",
            "./wasm/dotnet/benchmark.js",
        ],
        preload: dotnetPreloads("aot"),
        iterations: 15,
        worstCaseCount: 2,
        tags: ["default", "Wasm", "dotnet"],
    }),
    // J2CL
    new AsyncBenchmark({
        name: "j2cl-box2d-wasm",
        files: [
            "./wasm/j2cl-box2d/benchmark.js",
            "./wasm/j2cl-box2d/build/Box2dBenchmark_j2wasm_entry.js",
        ],
        preload: {
            wasmBinary: "./wasm/j2cl-box2d/build/Box2dBenchmark_j2wasm_binary.wasm",
        },
        iterations: 40,
        tags: ["default", "Wasm"],
    }),
];


const PRISM_JS_FILES = [
    "./utils/StartupBenchmark.js",
    "./prismjs/benchmark.js",
];
const PRISM_JS_PRELOADS = {
    SAMPLE_CPP: "./prismjs/data/sample.cpp",
    SAMPLE_CSS: "./prismjs/data/sample.css",
    SAMPLE_HTML: "./prismjs/data/sample.html",
    SAMPLE_JS: "./prismjs/data/sample.js",
    SAMPLE_JSON: "./prismjs/data/sample.json",
    SAMPLE_MD: "./prismjs/data/sample.md",
    SAMPLE_PY: "./prismjs/data/sample.py",
    SAMPLE_SQL: "./prismjs/data/sample.sql",
    SAMPLE_TS: "./prismjs/data/sample.ts",
};
const PRISM_JS_TAGS = ["js", "parser", "regexp", "startup", "prismjs"];
BENCHMARKS.push(
    new AsyncBenchmark({
        name: "prismjs-startup-es6",
        files: PRISM_JS_FILES,
        preload: {
            // Use non-minified bundle for better local profiling.
            // BUNDLE: "./prismjs/dist/bundle.es6.js",
            BUNDLE: "./prismjs/dist/bundle.es6.min.js",
            ...PRISM_JS_PRELOADS,
        },
        tags: ["default", ...PRISM_JS_TAGS, "es6"],
    }),
    new AsyncBenchmark({
        name: "prismjs-startup-es5",
        files: PRISM_JS_FILES,
        preload: {
            // Use non-minified bundle for better local profiling.
            // BUNDLE: "./prismjs/dist/bundle.es5.js",
            BUNDLE: "./prismjs/dist/bundle.es5.min.js",
            ...PRISM_JS_PRELOADS,
        },
        tags: [...PRISM_JS_TAGS, "es5"],
    }),
);

const INTL_TESTS = [
    "DateTimeFormat",
    "ListFormat",
    "RelativeTimeFormat",
    "NumberFormat",
    "PluralRules",
];
const INTL_TAGS = ["js", "internationalization"]
const INTL_BENCHMARKS = [];
for (const test of INTL_TESTS) {
    const benchmark = new AsyncBenchmark({
        name: `${test}-intl`,
        files: [
            "./intl/src/helper.js",
            `./intl/src/${test}.js`,
            "./intl/benchmark.js",
        ],
        iterations: 2,
        worstCaseCount: 1,
        deterministicRandom: true,
        tags: INTL_TAGS,
    });
    INTL_BENCHMARKS.push(benchmark);
}
BENCHMARKS.push(
    new GroupedBenchmark({
            name: "intl",
            tags: INTL_TAGS,
        }, INTL_BENCHMARKS));



// SunSpider tests
const SUNSPIDER_TESTS = [
    "3d-cube",
    "3d-raytrace",
    "base64",
    "crypto-aes",
    "crypto-md5",
    "crypto-sha1",
    "date-format-tofte",
    "date-format-xparb",
    "n-body",
    "regex-dna",
    "string-unpack-code",
    "tagcloud",
];
let SUNSPIDER_BENCHMARKS = [];
for (const test of SUNSPIDER_TESTS) {
    SUNSPIDER_BENCHMARKS.push(new DefaultBenchmark({
        name: `${test}-SP`,
        files: [
            `./SunSpider/${test}.js`
        ],
        tags: [],
    }));
}
BENCHMARKS.push(new GroupedBenchmark({
    name: "Sunspider",
    tags: ["default", "js", "SunSpider"],
}, SUNSPIDER_BENCHMARKS))

// WTB (Web Tooling Benchmark) tests
const WTB_TESTS = {
    "acorn": true,
    "babel": true,
    "babel-minify": true,
    "babylon": true,
    "chai": true,
    "espree": true,
    "esprima-next": true,
    // Disabled: Converting ES5 code to ES6+ is no longer a realistic scenario.
    "lebab": false,
    "postcss": true,
    "prettier": true,
    "source-map": true,
};
const WPT_FILES = [
  "angular-material-20.1.6.css",
  "backbone-1.6.1.js",
  "bootstrap-5.3.7.css",
  "foundation-6.9.0.css",
  "jquery-3.7.1.js",
  "lodash.core-4.17.21.js",
  "lodash-4.17.4.min.js.map",
  "mootools-core-1.6.0.js",
  "preact-8.2.5.js",
  "preact-10.27.1.min.module.js.map",
  "redux-5.0.1.min.js",
  "redux-5.0.1.esm.js",
  "source-map.min-0.5.7.js.map",
  "source-map/lib/mappings.wasm",
  "speedometer-es2015-test-2.0.js",
  "todomvc/react/app.jsx",
  "todomvc/react/footer.jsx",
  "todomvc/react/todoItem.jsx",
  "todomvc/typescript-angular.ts",
  "underscore-1.13.7.js",
  "underscore-1.13.7.min.js.map",
  "vue-3.5.18.runtime.esm-browser.js",
].reduce((acc, file) => {
        acc[file] = `./web-tooling-benchmark/third_party/${file}`;
        return acc
}, Object.create(null));


for (const [name, enabled] of Object.entries(WTB_TESTS)) {
    const tags =  ["js", "WTB"];
    if (enabled)
        tags.push("Default");
    BENCHMARKS.push(new AsyncBenchmark({
        name: `${name}-wtb`,
        files: [
            `./web-tooling-benchmark/dist/${name}.bundle.js`,
            "./web-tooling-benchmark/benchmark.js",
        ],
        preload: {
            BUNDLE: `./web-tooling-benchmark/dist/${name}.bundle.js`,
            ...WPT_FILES,
        },
        iterations: 15,
        worstCaseCount: 2,
        allowUtf16: true,
        tags: tags,
    }));
}


const benchmarksByName = new Map();
const benchmarksByTag = new Map();

for (const benchmark of BENCHMARKS) {
    const name = benchmark.name.toLowerCase();

    if (benchmarksByName.has(name))
        throw new Error(`Duplicate benchmark with name "${name}}"`);
    else
        benchmarksByName.set(name, benchmark);

    for (const tag of benchmark.tags) {
        if (benchmarksByTag.has(tag))
            benchmarksByTag.get(tag).push(benchmark);
        else
            benchmarksByTag.set(tag, [benchmark]);
    }
}


function processTestList(testList) {
    let benchmarkNames = [];
    let benchmarks = [];

    if (testList instanceof Array)
        benchmarkNames = testList;
    else
        benchmarkNames = testList.split(/[\s,]/);

    for (let name of benchmarkNames) {
        name = name.toLowerCase();
        if (benchmarksByTag.has(name))
            benchmarks = benchmarks.concat(findBenchmarksByTag(name));
        else
            benchmarks.push(findBenchmarkByName(name));
    }
    return benchmarks;
}


function findBenchmarkByName(name) {
    const benchmark = benchmarksByName.get(name.toLowerCase());

    if (!benchmark)
        throw new Error(`Couldn't find benchmark named "${name}"`);

    return benchmark;
}


function findBenchmarksByTag(tag, excludeTags) {
    let benchmarks = benchmarksByTag.get(tag.toLowerCase());
    if (!benchmarks) {
        const validTags = Array.from(benchmarksByTag.keys()).join(", ");
        throw new Error(`Couldn't find tag named: ${tag}.\n Choices are ${validTags}`);
    }
    if (excludeTags) {
        benchmarks = benchmarks.filter(benchmark => {
            return !benchmark.hasAnyTag(...excludeTags);
        });
    }
    return benchmarks;
}


let benchmarks = [];
const defaultDisabledTags = [];
// FIXME: add better support to run Worker tests in shells.
if (!isInBrowser)
    defaultDisabledTags.push("WorkerTests");

if (JetStreamParams.testList.length) {
    benchmarks = processTestList(JetStreamParams.testList);
} else {
    benchmarks = findBenchmarksByTag("Default", defaultDisabledTags)
}

this.JetStream = new Driver(benchmarks);


JetStream.initialize()
    .then(() => JetStream.start())
    .then(() => print("JETSTREAM_RUN_COMPLETE"))
    .catch((error) => print("JetStream2 failed:", error && error.stack ? error.stack : error));
undefined;
