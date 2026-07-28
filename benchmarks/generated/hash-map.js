
const isInBrowser = false;
globalThis.document = {
    getElementById() { return { innerHTML: "" }; }
};
globalThis.testList = "hash-map";
globalThis.testIterationCount = 5;
globalThis.RAMification = false;
globalThis.__jetstreamResources = {"./simple/hash-map.js":"/*\r\n *  Licensed to the Apache Software Foundation (ASF) under one or more\r\n *  contributor license agreements.  See the NOTICE below for additional\r\n *  information regarding copyright ownership.\r\n *  The ASF licenses this file to You under the Apache License, Version 2.0\r\n *  (the \"License\"); you may not use this file except in compliance with\r\n *  the License.  You may obtain a copy of the License at\r\n *\r\n *     http://www.apache.org/licenses/LICENSE-2.0\r\n *\r\n *  Unless required by applicable law or agreed to in writing, software\r\n *  distributed under the License is distributed on an \"AS IS\" BASIS,\r\n *  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.\r\n *  See the License for the specific language governing permissions and\r\n *  limitations under the License.\r\n */\r\n\r\n/******* NOTICE *********\r\n\r\nApache Harmony\r\nCopyright 2006, 2010 The Apache Software Foundation.\r\n\r\nThis product includes software developed at\r\nThe Apache Software Foundation (http://www.apache.org/).\r\n\r\nPortions of Apache Harmony were originally developed by\r\nIntel Corporation and are licensed to the Apache Software\r\nFoundation under the \"Software Grant and Corporate Contribution\r\nLicense Agreement\" and for which the following copyright notices\r\napply\r\n         (C) Copyright 2005 Intel Corporation\r\n         (C) Copyright 2005-2006 Intel Corporation\r\n         (C) Copyright 2006 Intel Corporation\r\n\r\n\r\nThe following copyright notice(s) were affixed to portions of the code\r\nwith which this file is now or was at one time distributed\r\nand are placed here unaltered.\r\n\r\n(C) Copyright 1997,2004 International Business Machines Corporation.\r\nAll rights reserved.\r\n\r\n(C) Copyright IBM Corp. 2003. \r\n\r\n\r\nThis software contains code derived from UNIX V7, Copyright(C)\r\nCaldera International Inc.\r\n\r\n************************/\r\n\r\n// This code is a manual translation of Apache Harmony's HashMap class to\r\n// JavaScript.\r\n\r\nvar HashMap = (function() {\r\n    var DEFAULT_SIZE = 16;\r\n    \r\n    function calculateCapacity(x)\r\n    {\r\n        if (x >= 1 << 30)\r\n            return 1 << 30;\r\n        if (x == 0)\r\n            return 16;\r\n        x = x - 1;\r\n        x |= x >> 1;\r\n        x |= x >> 2;\r\n        x |= x >> 4;\r\n        x |= x >> 8;\r\n        x |= x >> 16;\r\n        return x + 1;\r\n    }\r\n    \r\n    function computeHashCode(key)\r\n    {\r\n        switch (typeof key) {\r\n        case \"undefined\":\r\n            return 0;\r\n        case \"object\":\r\n            if (!key)\r\n                return 0;\r\n        case \"function\":\r\n            return key.hashCode();\r\n        case \"boolean\":\r\n            return key | 0;\r\n        case \"number\":\r\n            if ((key | 0) == key)\r\n                return key;\r\n            key = \"\" + key; // Compute string hash of the double. It's the best we can do.\r\n        case \"string\":\r\n            // source: http://en.wikipedia.org/wiki/Java_hashCode()#Sample_implementations_of_the_java.lang.String_algorithm\r\n            var h = 0;\r\n            var len = key.length;\r\n            for (var index = 0; index < len; index++) {\r\n                h = (((31 * h) | 0) + key.charCodeAt(index)) | 0;\r\n            }\r\n            return h;\r\n        default:\r\n            throw new Error(\"Internal error: Bad JavaScript value type\");\r\n        }\r\n    }\r\n    \r\n    function equals(a, b)\r\n    {\r\n        if (typeof a != typeof b)\r\n            return false;\r\n        switch (typeof a) {\r\n        case \"object\":\r\n            if (!a)\r\n                return !b;\r\n        case \"function\":\r\n            switch (typeof b) {\r\n            case \"object\":\r\n            case \"function\":\r\n                return a.equals(b);\r\n            default:\r\n                return false;\r\n            }\r\n        default:\r\n            return a == b;\r\n        }\r\n    }\r\n    \r\n    function Entry(key, hash, value)\r\n    {\r\n        this._key = key;\r\n        this._value = value;\r\n        this._origKeyHash = hash;\r\n        this._next = null;\r\n    }\r\n\r\n    Entry.prototype = {\r\n        clone: function()\r\n        {\r\n            var result = new Entry(this._key, this._hash, this._value);\r\n            if (this._next)\r\n                result._next = this._next.clone();\r\n            return result;\r\n        },\r\n        \r\n        toString: function()\r\n        {\r\n            return this._key + \"=\" + this._value;\r\n        },\r\n        \r\n        get key()\r\n        {\r\n            return this._key;\r\n        },\r\n        \r\n        get value()\r\n        {\r\n            return this._value;\r\n        }\r\n    };\r\n    \r\n    function AbstractMapIterator(map)\r\n    {\r\n        this._associatedMap = map;\r\n        this._expectedModCount = map._modCount;\r\n        this._futureEntry = null;\r\n        this._currentEntry = null;\r\n        this._prevEntry = null;\r\n        this._position = 0;\r\n    }\r\n    \r\n    AbstractMapIterator.prototype = {\r\n        hasNext: function()\r\n        {\r\n            if (this._futureEntry)\r\n                return true;\r\n            while (this._position < this._associatedMap._elementData.length) {\r\n                if (!this._associatedMap._elementData[this._position])\r\n                    this._position++;\r\n                else\r\n                    return true;\r\n            }\r\n            return false;\r\n        },\r\n        \r\n        _checkConcurrentMod: function()\r\n        {\r\n            if (this._expectedModCount != this._associatedMap._modCount)\r\n                throw new Error(\"Concurrent HashMap modification detected\");\r\n        },\r\n        \r\n        _makeNext: function()\r\n        {\r\n            this._checkConcurrentMod();\r\n            if (!this.hasNext())\r\n                throw new Error(\"No such element\");\r\n            if (!this._futureEntry) {\r\n                this._currentEntry = this._associatedMap._elementData[this._position++];\r\n                this._futureEntry = this._currentEntry._next;\r\n                this._prevEntry = null;\r\n                return;\r\n            }\r\n            if (this._currentEntry)\r\n                this._prevEntry = this._currentEntry;\r\n            this._currentEntry = this._futureEntry;\r\n            this._futureEntry = this._futureEntry._next;\r\n        },\r\n        \r\n        remove: function()\r\n        {\r\n            this._checkConcurrentMod();\r\n            if (!this._currentEntry)\r\n                throw new Error(\"Illegal state\");\r\n            if (!this._prevEntry) {\r\n                var index = this._currentEntry._origKeyHash & (this._associatedMap._elementData.length - 1);\r\n                this._associatedMap._elementData[index] = this._associatedMap._elementData[index]._next;\r\n            } else\r\n                this._prevEntry._next = this._currentEntry._next;\r\n            this._currentEntry = null;\r\n            this._expectedModCount++;\r\n            this._associatedMap._modCount++;\r\n            this._associatedMap._elementCount--;\r\n        }\r\n    };\r\n    \r\n    function EntryIterator(map)\r\n    {\r\n        AbstractMapIterator.call(this, map);\r\n    }\r\n    \r\n    EntryIterator.prototype = {\r\n        next: function()\r\n        {\r\n            this._makeNext();\r\n            return this._currentEntry;\r\n        }\r\n    };\r\n    EntryIterator.prototype.__proto__ = AbstractMapIterator.prototype;\r\n    \r\n    function KeyIterator(map)\r\n    {\r\n        AbstractMapIterator.call(this, map);\r\n    }\r\n    \r\n    KeyIterator.prototype = {\r\n        next: function()\r\n        {\r\n            this.makeNext();\r\n            return this._currentEntry._key;\r\n        }\r\n    };\r\n    KeyIterator.prototype.__proto__ = AbstractMapIterator.prototype;\r\n    \r\n    function ValueIterator(map)\r\n    {\r\n        AbstractMapIterator.call(this, map);\r\n    }\r\n    \r\n    ValueIterator.prototype = {\r\n        next: function()\r\n        {\r\n            this.makeNext();\r\n            return this._currentEntry._value;\r\n        }\r\n    };\r\n    ValueIterator.prototype.__proto__ = AbstractMapIterator.prototype;\r\n    \r\n    function EntrySet(map)\r\n    {\r\n        this._associatedMap = map;\r\n    }\r\n    \r\n    EntrySet.prototype = {\r\n        size: function()\r\n        {\r\n            return this._associatedMap._elementCount;\r\n        },\r\n        \r\n        clear: function()\r\n        {\r\n            this._associatedMap.clear();\r\n        },\r\n        \r\n        remove: function(object)\r\n        {\r\n            var entry = this._associatedMap._getEntry(object.key);\r\n            if (!entry)\r\n                return false;\r\n            if (!equals(entry._value, object.value))\r\n                return false;\r\n            this._associatedMap._removeEntry(entry);\r\n            return true;\r\n        },\r\n        \r\n        contains: function(object)\r\n        {\r\n            var entry = this._associatedMap._getEntry(object.key);\r\n            if (!entry)\r\n                return false;\r\n            return equals(entry._value, object.value);\r\n        },\r\n        \r\n        iterator: function()\r\n        {\r\n            return new EntryIterator(this._associatedMap);\r\n        }\r\n    };\r\n    \r\n    function KeySet(map)\r\n    {\r\n        this._associatedMap = map;\r\n    }\r\n    \r\n    KeySet.prototype = {\r\n        contains: function(object)\r\n        {\r\n            return this._associatedMap.contains(object);\r\n        },\r\n        \r\n        size: function()\r\n        {\r\n            return this._associatedMap._elementCount;\r\n        },\r\n        \r\n        clear: function()\r\n        {\r\n            this._associatedMap.clear();\r\n        },\r\n        \r\n        remove: function(key)\r\n        {\r\n            return !!this._associatedMap.remove(key);\r\n        },\r\n        \r\n        iterator: function()\r\n        {\r\n            return new KeyIterator(this._associatedMap);\r\n        }\r\n    };\r\n    \r\n    function ValueCollection(map)\r\n    {\r\n        this._associatedMap = map;\r\n    }\r\n    \r\n    ValueCollection.prototype = {\r\n        contains: function(object)\r\n        {\r\n            return this._associatedMap.containsValue(object);\r\n        },\r\n        \r\n        size: function()\r\n        {\r\n            return this._associatedMap._elementCount;\r\n        },\r\n        \r\n        clear: function()\r\n        {\r\n            this._associatedMap.clear();\r\n        },\r\n        \r\n        iterator: function()\r\n        {\r\n            return new ValueIterator(this._associatedMap);\r\n        }\r\n    };\r\n    \r\n    function HashMap(capacity, loadFactor)\r\n    {\r\n        if (capacity == null)\r\n            capacity = DEFAULT_SIZE;\r\n        if (loadFactor == null)\r\n            loadFactor = 0.75;\r\n        \r\n        if (capacity < 0)\r\n            throw new Error(\"Invalid argument to HashMap constructor: capacity is negative\");\r\n        if (loadFactor <= 0)\r\n            throw new Error(\"Invalid argument to HashMap constructor: loadFactor is not positive\");\r\n        \r\n        this._capacity = calculateCapacity(capacity);\r\n        this._elementCount = 0;\r\n        this._elementData = new Array(this.capacity);\r\n        this._loadFactor = loadFactor;\r\n        this._modCount = 0;\r\n        this._computeThreshold();\r\n    }\r\n    \r\n    HashMap.prototype = {\r\n        _computeThreshold: function()\r\n        {\r\n            this._threshold = (this._elementData.length * this._loadFactor) | 0;\r\n        },\r\n        \r\n        clear: function()\r\n        {\r\n            if (!this._elementCount)\r\n                return;\r\n            \r\n            this._elementCount = 0;\r\n            for (var i = this._elementData.length; i--;)\r\n                this._elementData[i] = null;\r\n            this._modCount++;\r\n        },\r\n        \r\n        clone: function()\r\n        {\r\n            var result = new HashMap(this._elementData.length, this._loadFactor);\r\n            result.putAll(this);\r\n            return result;\r\n        },\r\n        \r\n        containsKey: function(key)\r\n        {\r\n            return !!this._getEntry(key);\r\n        },\r\n        \r\n        containsValue: function(value)\r\n        {\r\n            for (var i = this._elementData.length; i--;) {\r\n                for (var entry = this._elementData[i]; entry; entry = entry._next) {\r\n                    if (equals(value, entry._value))\r\n                        return true;\r\n                }\r\n            }\r\n            return false;\r\n        },\r\n        \r\n        entrySet: function()\r\n        {\r\n            return new EntrySet(this);\r\n        },\r\n        \r\n        get: function(key)\r\n        {\r\n            var entry = this._getEntry(key);\r\n            if (entry)\r\n                return entry._value;\r\n            return null;\r\n        },\r\n        \r\n        _getEntry: function(key)\r\n        {\r\n            var hash = computeHashCode(key);\r\n            var index = hash & (this._elementData.length - 1);\r\n            return this._findKeyEntry(key, index, hash);\r\n        },\r\n        \r\n        _findKeyEntry: function(key, index, keyHash)\r\n        {\r\n            var entry = this._elementData[index];\r\n            while (entry && (entry._origKeyHash != keyHash || !equals(key, entry._key)))\r\n                entry = entry._next;\r\n            return entry;\r\n        },\r\n        \r\n        isEmpty: function()\r\n        {\r\n            return !this._elementCount;\r\n        },\r\n        \r\n        keySet: function()\r\n        {\r\n            return new KeySet(this);\r\n        },\r\n        \r\n        put: function(key, value)\r\n        {\r\n            var hash = computeHashCode(key);\r\n            var index = hash & (this._elementData.length - 1);\r\n            var entry = this._findKeyEntry(key, index, hash);\r\n            if (!entry) {\r\n                this._modCount++;\r\n                entry = this._createHashedEntry(key, index, hash);\r\n                if (++this._elementCount > this._threshold)\r\n                    this._rehash();\r\n            }\r\n            \r\n            var result = entry._value;\r\n            entry._value = value;\r\n            return result;\r\n        },\r\n        \r\n        _createHashedEntry: function(key, index, hash)\r\n        {\r\n            var entry = new Entry(key, hash, null);\r\n            entry._next = this._elementData[index];\r\n            this._elementData[index] = entry;\r\n            return entry;\r\n        },\r\n        \r\n        putAll: function(map)\r\n        {\r\n            if (map.isEmpty())\r\n                return;\r\n            for (var iter = map.entrySet().iterator(); iter.hasNext();) {\r\n                var entry = iter.next();\r\n                put(entry.key, entry.value);\r\n            }\r\n        },\r\n        \r\n        _rehash: function(capacity)\r\n        {\r\n            if (capacity == null)\r\n                capacity = this._elementData.length;\r\n            \r\n            var length = calculateCapacity(!capacity ? 1 : capacity << 1);\r\n            var newData = new Array(length);\r\n            for (var i = 0; i < this._elementData.length; ++i) {\r\n                var entry = this._elementData[i];\r\n                this._elementData[i] = null;\r\n                while (entry) {\r\n                    var index = entry._origKeyHash & (length - 1);\r\n                    var next = entry._next;\r\n                    entry._next = newData[index];\r\n                    newData[index] = entry;\r\n                    entry = next;\r\n                }\r\n            }\r\n            this._elementData = newData;\r\n            this._computeThreshold();\r\n        },\r\n        \r\n        remove: function(key)\r\n        {\r\n            var entry = this._removeEntryForKey(key);\r\n            if (!entry)\r\n                return null;\r\n            return entry._value;\r\n        },\r\n        \r\n        _removeEntry: function(entry)\r\n        {\r\n            var index = entry._origKeyHash & (this._elementData.length - 1);\r\n            var current = this._elementData[index];\r\n            if (current == entry)\r\n                this._elementData[index] = entry._next;\r\n            else {\r\n                while (current._next != entry)\r\n                    current = current._next;\r\n                current._next = entry._next;\r\n            }\r\n            this._modCount++;\r\n            this._elementCount--;\r\n        },\r\n        \r\n        _removeEntryForKey: function(key)\r\n        {\r\n            var hash = computeHashCode(key);\r\n            var index = hash & (this._elementData.length - 1);\r\n            var entry = this._elementData[index];\r\n            var last = null;\r\n            while (entry != null && !(entry._origKeyHash == hash && equals(key, entry._key))) {\r\n                last = entry;\r\n                entry = entry._next;\r\n            }\r\n            if (!entry)\r\n                return null;\r\n            if (!last)\r\n                this._elementData[index] = entry._next;\r\n            else\r\n                last._next = entry._next;\r\n            this._modCount++;\r\n            this._elementCount--;\r\n            return entry;\r\n        },\r\n        \r\n        size: function()\r\n        {\r\n            return this._elementCount;\r\n        },\r\n        \r\n        values: function()\r\n        {\r\n            return new ValueCollection(this);\r\n        }\r\n    };\r\n    \r\n    return HashMap;\r\n})();\r\n\r\nfunction run() {\r\n    var map = new HashMap();\r\n    var COUNT = 90000;\r\n\r\n    for (var i = 0; i < COUNT; ++i)\r\n        map.put(i, 42);\r\n\r\n    var result = 0;\r\n    for (var j = 0; j < 5; ++j) {\r\n        for (var i = 0; i < COUNT; ++i)\r\n            result += map.get(i);\r\n    }\r\n\r\n    var keySum = 0;\r\n    var valueSum = 0;\r\n    for (var iterator = map.entrySet().iterator(); iterator.hasNext();) {\r\n        var entry = iterator.next();\r\n        keySum += entry.key;\r\n        valueSum += entry.value;\r\n    }\r\n\r\n    if (result !== 42 * COUNT * 5)\r\n        throw \"Error: result = \" + result;\r\n    if (keySum !== ((COUNT * (COUNT + 1) / 2) - COUNT))\r\n        throw \"Error: keySum = \" + keySum;\r\n    if (valueSum !== 42 * COUNT)\r\n        throw \"Error: valueSum = \" + valueSum;\r\n}\r\n\r\nclass Benchmark {\r\n    runIteration() {\r\n        run();\r\n    }\r\n}\r\n\r\n"};
globalThis.readFile = function (name) {
    const normalized = String(name).replaceAll("\\", "/");
    if (!Object.prototype.hasOwnProperty.call(__jetstreamResources, normalized))
        throw new Error("JetStream resource not embedded: " + normalized);
    return __jetstreamResources[normalized];
};
globalThis.runString = function (source) {
    if (source)
        __agentjsLoadString(source);
    const shellRealm = {
        print,
        loadString(text) { return __agentjsLoadString(text); }
    };
    Object.defineProperty(shellRealm, "console", {
        get() { return globalThis.console; },
        set(_) {}
    });
    Object.defineProperty(shellRealm, "top", {
        get() { return globalThis.top; },
        set(value) { globalThis.top = value; }
    });
    return shellRealm;
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
this.currentResolve = null;
this.currentReject = null;

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

        let start = Date.now();
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

        let totalTime = Date.now() - start;
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
            let scripts = string;
            let globalObject = runString("");
            globalObject.console = {log:globalObject.print}
            globalObject.top = {
                currentResolve,
                currentReject
            };
            for (let script of scripts)
                globalObject.loadString(script);
            return globalObject;
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

class JetStreamBenchmarkBase {
    constructor(plan)
    {
        this.plan = plan;
        this.iterations = testIterationCount || plan.iterations || defaultIterationCount;
        this.isAsync = !!plan.isAsync;

        this.scripts = null;

        this._resourcesPromise = null;
        this.fetchResources();
    }

    get name() { return this.plan.name; }

    get runnerCode() {
        return `
            let __benchmark = new Benchmark(${this.iterations});
            let results = [];
            for (let i = 0; i < ${this.iterations}; i++) {
                if (__benchmark.prepareForNextIteration)
                    __benchmark.prepareForNextIteration();

                let start = Date.now();
                __benchmark.runIteration();
                let end = Date.now();

                results.push(Math.max(1, end - start));
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

        addScript(`globalThis.performance = {now: Date.now.bind(Date)};`);

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
        addScript(this.runnerCode);

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

class DefaultBenchmark extends JetStreamBenchmarkBase {
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

class WSLBenchmark extends JetStreamBenchmarkBase {
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

class WasmBenchmark extends JetStreamBenchmarkBase {
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

this.JetStream = new Driver();

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
