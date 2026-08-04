
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
var testList = "mobx";
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
    testList: "mobx",
};
var __jetstreamResources = {"./mobx/benchmark.js":"/*\n * Copyright (C) 2025 Apple Inc. All rights reserved.\n *\n * Redistribution and use in source and binary forms, with or without\n * modification, are permitted provided that the following conditions\n * are met:\n * 1. Redistributions of source code must retain the above copyright\n *    notice, this list of conditions and the following disclaimer.\n * 2. Redistributions in binary form must reproduce the above copyright\n *    notice, this list of conditions and the following disclaimer in the\n *    documentation and/or other materials provided with the distribution.\n *\n * THIS SOFTWARE IS PROVIDED BY APPLE INC. ``AS IS'' AND ANY\n * EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE\n * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR\n * PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL APPLE INC. OR\n * CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,\n * EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,\n * PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR\n * PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY\n * OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\n * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE\n * OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\n */\n\nconst EXPECTED_LOG_LENGTH = 6824;\n\nclass Benchmark extends StartupBenchmark {\n  lastResult;\n\n  constructor({iterationCount}) {\n    super({\n      iterationCount,\n      expectedCacheCommentCount: 464,\n      sourceCodeReuseCount: 8,\n    });\n  }\n\n  runIteration(iteration) {\n    // Module is loaded into PrismJSBenchmark\n    let MobXBenchmark;\n    eval(this.iterationSourceCodes[iteration]);\n    this.lastResult = MobXBenchmark.runTest();\n  }\n\n  validate() {\n    console.assert(\n      this.lastResult.length === EXPECTED_LOG_LENGTH,\n      `Expected this.lastResult.length to be ${EXPECTED_LOG_LENGTH}, but got ${this.lastResult.length}`\n    );\n  }\n}\n","./mobx/dist/bundle.es6.min.js":"(()=>{var e={d:(t,n)=>{for(var r in n)e.o(n,r)&&!e.o(t,r)&&Object.defineProperty(t,r,{enumerable:!0,get:n[r]})}};e.g=function(){if(\"object\"==typeof globalThis)return globalThis;try{return this||new Function(\"return this\")()}catch(e){if(\"object\"==typeof window)return window}}(),e.o=(e,t)=>Object.prototype.hasOwnProperty.call(e,t),e.r=e=>{\"undefined\"!=typeof Symbol&&Symbol.toStringTag&&Object.defineProperty(e,Symbol.toStringTag,{value:\"Module\"}),Object.defineProperty(e,\"__esModule\",{value:!0})};var t={};(()=>{\"use strict\";e.r(t),e.d(t,{runTest:()=>br});function n(e){\n/*ThouShaltNotCache*/\nfor(var t=arguments.length,n=new Array(t>1?t-1:0),r=1;r<t;r++)n[r-1]=arguments[r];throw new Error(\"number\"==typeof e?\"[MobX] minified error nr: \"+e+(n.length?\" \"+n.map(String).join(\",\"):\"\")+\". Find the full error at: https://github.com/mobxjs/mobx/blob/main/packages/mobx/src/errors.ts\":\"[MobX] \"+e)}var r={};function i(){\n/*ThouShaltNotCache*/\nreturn\"undefined\"!=typeof globalThis?globalThis:\"undefined\"!=typeof window?window:void 0!==e.g?e.g:\"undefined\"!=typeof self?self:r}var o=Object.assign,s=Object.getOwnPropertyDescriptor,a=Object.defineProperty,u=Object.prototype,c=[];Object.freeze(c);var l={};Object.freeze(l);var h=\"undefined\"!=typeof Proxy,f=Object.toString();function d(){\n/*ThouShaltNotCache*/\nh||n(\"Proxy not available\")}function _(e){\n/*ThouShaltNotCache*/\nvar t=!1;return function(){\n/*ThouShaltNotCache*/\nif(!t)return t=!0,e.apply(this,arguments)}}var v=function(){};function p(e){\n/*ThouShaltNotCache*/\nreturn\"function\"==typeof e}function g(e){switch(typeof e){case\"string\":case\"symbol\":case\"number\":return!0}return!1}function b(e){\n/*ThouShaltNotCache*/\nreturn null!==e&&\"object\"==typeof e}function y(e){\n/*ThouShaltNotCache*/\nif(!b(e))return!1;var t=Object.getPrototypeOf(e);if(null==t)return!0;var n=Object.hasOwnProperty.call(t,\"constructor\")&&t.constructor;return\"function\"==typeof n&&n.toString()===f}function m(e){\n/*ThouShaltNotCache*/\nvar t=null==e?void 0:e.constructor;return!!t&&(\"GeneratorFunction\"===t.name||\"GeneratorFunction\"===t.displayName)}function O(e,t,n){\n/*ThouShaltNotCache*/\na(e,t,{enumerable:!1,writable:!0,configurable:!0,value:n})}function w(e,t,n){\n/*ThouShaltNotCache*/\na(e,t,{enumerable:!1,writable:!1,configurable:!0,value:n})}function S(e,t){\n/*ThouShaltNotCache*/\nvar n=\"isMobX\"+e;return t.prototype[n]=!0,function(e){\n/*ThouShaltNotCache*/\nreturn b(e)&&!0===e[n]}}function A(e){\n/*ThouShaltNotCache*/\nreturn null!=e&&\"[object Map]\"===Object.prototype.toString.call(e)}function k(e){\n/*ThouShaltNotCache*/\nreturn null!=e&&\"[object Set]\"===Object.prototype.toString.call(e)}var j=void 0!==Object.getOwnPropertySymbols;var x=\"undefined\"!=typeof Reflect&&Reflect.ownKeys?Reflect.ownKeys:j?function(e){\n/*ThouShaltNotCache*/\nreturn Object.getOwnPropertyNames(e).concat(Object.getOwnPropertySymbols(e))}:Object.getOwnPropertyNames;function M(e){\n/*ThouShaltNotCache*/\nreturn null===e?null:\"object\"==typeof e?\"\"+e:e}function P(e,t){\n/*ThouShaltNotCache*/\nreturn u.hasOwnProperty.call(e,t)}var T=Object.getOwnPropertyDescriptors||function(e){\n/*ThouShaltNotCache*/\nvar t={};return x(e).forEach(function(n){\n/*ThouShaltNotCache*/\nt[n]=s(e,n)}),t};function V(e,t){\n/*ThouShaltNotCache*/\nreturn!!(e&t)}function E(e,t,n){\n/*ThouShaltNotCache*/\nreturn n?e|=t:e&=~t,e}function R(e,t){\n/*ThouShaltNotCache*/\n(null==t||t>e.length)&&(t=e.length);for(var n=0,r=Array(t);n<t;n++)r[n]=e[n];return r}function C(e,t){\n/*ThouShaltNotCache*/\nfor(var n=0;n<t.length;n++){var r=t[n];r.enumerable=r.enumerable||!1,r.configurable=!0,\"value\"in r&&(r.writable=!0),Object.defineProperty(e,I(r.key),r)}}function D(e,t,n){\n/*ThouShaltNotCache*/\nreturn t&&C(e.prototype,t),n&&C(e,n),Object.defineProperty(e,\"prototype\",{writable:!1}),e}function N(e,t){\n/*ThouShaltNotCache*/\nvar n=\"undefined\"!=typeof Symbol&&e[Symbol.iterator]||e[\"@@iterator\"];if(n)return(n=n.call(e)).next.bind(n);if(Array.isArray(e)||(n=function(e,t){\n/*ThouShaltNotCache*/\nif(e){if(\"string\"==typeof e)return R(e,t);var n={}.toString.call(e).slice(8,-1);return\"Object\"===n&&e.constructor&&(n=e.constructor.name),\"Map\"===n||\"Set\"===n?Array.from(e):\"Arguments\"===n||/^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(n)?R(e,t):void 0}}(e))||t&&e&&\"number\"==typeof e.length){n&&(e=n);var r=0;return function(){\n/*ThouShaltNotCache*/\nreturn r>=e.length?{done:!0}:{done:!1,value:e[r++]}}}throw new TypeError(\"Invalid attempt to iterate non-iterable instance.\\nIn order to be iterable, non-array objects must have a [Symbol.iterator]() method.\")}function L(){\n/*ThouShaltNotCache*/\nreturn L=Object.assign?Object.assign.bind():function(e){\n/*ThouShaltNotCache*/\nfor(var t=1;t<arguments.length;t++){var n=arguments[t];for(var r in n)({}).hasOwnProperty.call(n,r)&&(e[r]=n[r])}return e},L.apply(null,arguments)}function B(e,t){\n/*ThouShaltNotCache*/\ne.prototype=Object.create(t.prototype),e.prototype.constructor=e,U(e,t)}function U(e,t){\n/*ThouShaltNotCache*/\nreturn U=Object.setPrototypeOf?Object.setPrototypeOf.bind():function(e,t){\n/*ThouShaltNotCache*/\nreturn e.__proto__=t,e},U(e,t)}function I(e){\n/*ThouShaltNotCache*/\nvar t=function(e,t){\n/*ThouShaltNotCache*/\nif(\"object\"!=typeof e||!e)return e;var n=e[Symbol.toPrimitive];if(void 0!==n){var r=n.call(e,t||\"default\");if(\"object\"!=typeof r)return r;throw new TypeError(\"@@toPrimitive must return a primitive value.\")}return(\"string\"===t?String:Number)(e)}(e,\"string\");return\"symbol\"==typeof t?t:t+\"\"}var K=Symbol(\"mobx-stored-annotations\");function G(e){return Object.assign(\n/*ThouShaltNotCache*/\nfunction(t,n){\n/*ThouShaltNotCache*/\nif(z(n))return e.decorate_20223_(t,n);q(t,n,e)},e)}function q(e,t,n){\n/*ThouShaltNotCache*/\nP(e,K)||O(e,K,L({},e[K])),function(e){\n/*ThouShaltNotCache*/\nreturn e.annotationType_===Q}(n)||(e[K][t]=n)}function z(e){\n/*ThouShaltNotCache*/\nreturn\"object\"==typeof e&&\"string\"==typeof e.kind}var H=Symbol(\"mobx administration\"),W=function(){\n/*ThouShaltNotCache*/\nfunction e(e){\n/*ThouShaltNotCache*/\nvoid 0===e&&(e=\"Atom\"),this.name_=void 0,this.flags_=0,this.observers_=new Set,this.lastAccessedBy_=0,this.lowestObserverState_=Ye.NOT_TRACKING_,this.onBOL=void 0,this.onBUOL=void 0,this.name_=e}var t=e.prototype;return t.onBO=function(){\n/*ThouShaltNotCache*/\nthis.onBOL&&this.onBOL.forEach(function(e){\n/*ThouShaltNotCache*/\nreturn e()})},t.onBUO=function(){\n/*ThouShaltNotCache*/\nthis.onBUOL&&this.onBUOL.forEach(function(e){\n/*ThouShaltNotCache*/\nreturn e()})},t.reportObserved=function(){\n/*ThouShaltNotCache*/\nreturn mt(this)},t.reportChanged=function(){\n/*ThouShaltNotCache*/\nbt(),Ot(this),yt()},t.toString=function(){\n/*ThouShaltNotCache*/\nreturn this.name_},D(e,[{key:\"isBeingObserved\",get:function(){\n/*ThouShaltNotCache*/\nreturn V(this.flags_,e.isBeingObservedMask_)},set:function(t){\n/*ThouShaltNotCache*/\nthis.flags_=E(this.flags_,e.isBeingObservedMask_,t)}},{key:\"isPendingUnobservation\",get:function(){\n/*ThouShaltNotCache*/\nreturn V(this.flags_,e.isPendingUnobservationMask_)},set:function(t){\n/*ThouShaltNotCache*/\nthis.flags_=E(this.flags_,e.isPendingUnobservationMask_,t)}},{key:\"diffValue\",get:function(){\n/*ThouShaltNotCache*/\nreturn V(this.flags_,e.diffValueMask_)?1:0},set:function(t){\n/*ThouShaltNotCache*/\nthis.flags_=E(this.flags_,e.diffValueMask_,1===t)}}])}();W.isBeingObservedMask_=1,W.isPendingUnobservationMask_=2,W.diffValueMask_=4;var $=S(\"Atom\",W);function F(e,t,n){\n/*ThouShaltNotCache*/\nvoid 0===t&&(t=v),void 0===n&&(n=v);var r,i=new W(e);return t!==v&&Ht(Gt,i,t,r),n!==v&&zt(i,n),i}var X={identity:function(e,t){\n/*ThouShaltNotCache*/\nreturn e===t},structural:function(e,t){\n/*ThouShaltNotCache*/\nreturn ar(e,t)},default:function(e,t){\n/*ThouShaltNotCache*/\nreturn Object.is?Object.is(e,t):e===t?0!==e||1/e==1/t:e!=e&&t!=t},shallow:function(e,t){\n/*ThouShaltNotCache*/\nreturn ar(e,t,1)}};function Y(e,t,n){\n/*ThouShaltNotCache*/\nreturn tn(e)?e:Array.isArray(e)?Ce.array(e,{name:n}):y(e)?Ce.object(e,void 0,{name:n}):A(e)?Ce.map(e,{name:n}):k(e)?Ce.set(e,{name:n}):\"function\"!=typeof e||Bt(e)||Zt(e)?e:m(e)?Jt(e):Lt(n,e)}function J(e){\n/*ThouShaltNotCache*/\nreturn e}var Q=\"override\";function Z(e,t){\n/*ThouShaltNotCache*/\nreturn{annotationType_:e,options_:t,make_:ee,extend_:te,decorate_20223_:ne}}function ee(e,t,n,r){\n/*ThouShaltNotCache*/\nvar i;if(null!=(i=this.options_)&&i.bound)return null===this.extend_(e,t,n,!1)?0:1;if(r===e.target_)return null===this.extend_(e,t,n,!1)?0:2;if(Bt(n.value))return 1;var o=re(e,this,t,n,!1);return a(r,t,o),2}function te(e,t,n,r){\n/*ThouShaltNotCache*/\nvar i=re(e,this,t,n);return e.defineProperty_(t,i,r)}function ne(e,t){var r,i=t.kind,o=t.name,s=t.addInitializer,a=this,u=function(e){\n/*ThouShaltNotCache*/\nvar t,n,r,i;return qe(null!=(t=null==(n=a.options_)?void 0:n.name)?t:o.toString(),e,null!=(r=null==(i=a.options_)?void 0:i.autoAction)&&r)};return\"field\"==i?function(e){\n/*ThouShaltNotCache*/\nvar t,n=e;return Bt(n)||(n=u(n)),null!=(t=a.options_)&&t.bound&&((n=n.bind(this)).isMobxAction=!0),n}:\"method\"==i?(Bt(e)||(e=u(e)),null!=(r=this.options_)&&r.bound&&s(function(){\n/*ThouShaltNotCache*/\nvar e=this,t=e[o].bind(e);t.isMobxAction=!0,e[o]=t}),e):void n(\"Cannot apply '\"+a.annotationType_+\"' to '\"+String(o)+\"' (kind: \"+i+\"):\\n'\"+a.annotationType_+\"' can only be used on properties with a function value.\")}function re(e,t,n,r,i){\n/*ThouShaltNotCache*/\nvar o,s,a,u,c,l,h,f;void 0===i&&(i=_t.safeDescriptors),f=r,t.annotationType_,f.value;var d,_=r.value;null!=(o=t.options_)&&o.bound&&(_=_.bind(null!=(d=e.proxy_)?d:e.target_));return{value:qe(null!=(s=null==(a=t.options_)?void 0:a.name)?s:n.toString(),_,null!=(u=null==(c=t.options_)?void 0:c.autoAction)&&u,null!=(l=t.options_)&&l.bound?null!=(h=e.proxy_)?h:e.target_:void 0),configurable:!i||e.isPlainObject_,enumerable:!1,writable:!i}}function ie(e,t){\n/*ThouShaltNotCache*/\nreturn{annotationType_:e,options_:t,make_:oe,extend_:se,decorate_20223_:ae}}function oe(e,t,n,r){\n/*ThouShaltNotCache*/\nvar i;if(r===e.target_)return null===this.extend_(e,t,n,!1)?0:2;if(null!=(i=this.options_)&&i.bound&&(!P(e.target_,t)||!Zt(e.target_[t]))&&null===this.extend_(e,t,n,!1))return 0;if(Zt(n.value))return 1;var o=ue(e,this,t,n,!1,!1);return a(r,t,o),2}function se(e,t,n,r){\n/*ThouShaltNotCache*/\nvar i,o=ue(e,this,t,n,null==(i=this.options_)?void 0:i.bound);return e.defineProperty_(t,o,r)}function ae(e,t){\n/*ThouShaltNotCache*/\nvar n;var r=t.name,i=t.addInitializer;return Zt(e)||(e=Jt(e)),null!=(n=this.options_)&&n.bound&&i(function(){\n/*ThouShaltNotCache*/\nvar e=this,t=e[r].bind(e);t.isMobXFlow=!0,e[r]=t}),e}function ue(e,t,n,r,i,o){var s;\n/*ThouShaltNotCache*/\nvoid 0===o&&(o=_t.safeDescriptors),s=r,t.annotationType_,s.value;var a,u=r.value;(Zt(u)||(u=Jt(u)),i)&&((u=u.bind(null!=(a=e.proxy_)?a:e.target_)).isMobXFlow=!0);return{value:u,configurable:!o||e.isPlainObject_,enumerable:!1,writable:!o}}function ce(e,t){\n/*ThouShaltNotCache*/\nreturn{annotationType_:e,options_:t,make_:le,extend_:he,decorate_20223_:fe}}function le(e,t,n){\n/*ThouShaltNotCache*/\nreturn null===this.extend_(e,t,n,!1)?0:1}function he(e,t,n,r){\n/*ThouShaltNotCache*/\nreturn function(e,t,n,r){\n/*ThouShaltNotCache*/\nt.annotationType_,r.get;0}(0,this,0,n),e.defineComputedProperty_(t,L({},this.options_,{get:n.get,set:n.set}),r)}function fe(e,t){var n=this,r=t.name;return(0,t.addInitializer)(function(){\n/*ThouShaltNotCache*/\nvar t=Bn(this)[H],i=L({},n.options_,{get:e,context:this});i.name||(i.name=\"ObservableObject.\"+r.toString()),t.values_.set(r,new Xe(i))}),function(){\n/*ThouShaltNotCache*/\nreturn this[H].getObservablePropValue_(r)}}function de(e,t){\n/*ThouShaltNotCache*/\nreturn{annotationType_:e,options_:t,make_:_e,extend_:ve,decorate_20223_:pe}}function _e(e,t,n){\n/*ThouShaltNotCache*/\nreturn null===this.extend_(e,t,n,!1)?0:1}function ve(e,t,n,r){\n/*ThouShaltNotCache*/\nvar i,o;return function(e,t){\n/*ThouShaltNotCache*/\nt.annotationType_;0}(0,this),e.defineObservableProperty_(t,n.value,null!=(i=null==(o=this.options_)?void 0:o.enhancer)?i:Y,r)}function pe(e,t){var n=this,r=t.kind,i=t.name,o=new WeakSet;function s(e,t){\n/*ThouShaltNotCache*/\nvar r,s,a=Bn(e)[H],u=new Fe(t,null!=(r=null==(s=n.options_)?void 0:s.enhancer)?r:Y,\"ObservableObject.\"+i.toString(),!1);a.values_.set(i,u),o.add(e)}if(\"accessor\"==r)return{get:function(){\n/*ThouShaltNotCache*/\nreturn o.has(this)||s(this,e.get.call(this)),this[H].getObservablePropValue_(i)},set:function(e){\n/*ThouShaltNotCache*/\nreturn o.has(this)||s(this,e),this[H].setObservablePropValue_(i,e)},init:function(e){\n/*ThouShaltNotCache*/\nreturn o.has(this)||s(this,e),e}}}var ge=\"true\",be=ye();function ye(e){\n/*ThouShaltNotCache*/\nreturn{annotationType_:ge,options_:e,make_:me,extend_:Oe,decorate_20223_:we}}function me(e,t,n,r){\n/*ThouShaltNotCache*/\nvar i,o,s,u;if(n.get)return Be.make_(e,t,n,r);if(n.set){var c=qe(t.toString(),n.set);return r===e.target_?null===e.defineProperty_(t,{configurable:!_t.safeDescriptors||e.isPlainObject_,set:c})?0:2:(a(r,t,{configurable:!0,set:c}),2)}if(r!==e.target_&&\"function\"==typeof n.value)return m(n.value)?(null!=(u=this.options_)&&u.autoBind?Jt.bound:Jt).make_(e,t,n,r):(null!=(s=this.options_)&&s.autoBind?Lt.bound:Lt).make_(e,t,n,r);var l,h=!1===(null==(i=this.options_)?void 0:i.deep)?Ce.ref:Ce;\"function\"==typeof n.value&&null!=(o=this.options_)&&o.autoBind&&(n.value=n.value.bind(null!=(l=e.proxy_)?l:e.target_));return h.make_(e,t,n,r)}function Oe(e,t,n,r){\n/*ThouShaltNotCache*/\nvar i,o,s;if(n.get)return Be.extend_(e,t,n,r);if(n.set)return e.defineProperty_(t,{configurable:!_t.safeDescriptors||e.isPlainObject_,set:qe(t.toString(),n.set)},r);\"function\"==typeof n.value&&null!=(i=this.options_)&&i.autoBind&&(n.value=n.value.bind(null!=(s=e.proxy_)?s:e.target_));return(!1===(null==(o=this.options_)?void 0:o.deep)?Ce.ref:Ce).extend_(e,t,n,r)}function we(e,t){\n/*ThouShaltNotCache*/\nn(\"'\"+this.annotationType_+\"' cannot be used as a decorator\")}var Se={deep:!0,name:void 0,defaultDecorator:void 0,proxy:!0};function Ae(e){\n/*ThouShaltNotCache*/\nreturn e||Se}Object.freeze(Se);var ke=de(\"observable\"),je=de(\"observable.ref\",{enhancer:J}),xe=de(\"observable.shallow\",{enhancer:function(e,t,n){\n/*ThouShaltNotCache*/\nreturn null==e||Kn(e)||An(e)||Pn(e)||Rn(e)?e:Array.isArray(e)?Ce.array(e,{name:n,deep:!1}):y(e)?Ce.object(e,void 0,{name:n,deep:!1}):A(e)?Ce.map(e,{name:n,deep:!1}):k(e)?Ce.set(e,{name:n,deep:!1}):void 0}}),Me=de(\"observable.struct\",{enhancer:function(e,t){return ar(e,t)?t:e}}),Pe=G(ke);function Te(e){\n/*ThouShaltNotCache*/\nreturn!0===e.deep?Y:!1===e.deep?J:(t=e.defaultDecorator)&&null!=(n=null==(r=t.options_)?void 0:r.enhancer)?n:Y;var t,n,r}function Ve(e,t,n){\n/*ThouShaltNotCache*/\nreturn z(t)?ke.decorate_20223_(e,t):g(t)?void q(e,t,ke):tn(e)?e:y(e)?Ce.object(e,t,n):Array.isArray(e)?Ce.array(e,t):A(e)?Ce.map(e,t):k(e)?Ce.set(e,t):\"object\"==typeof e&&null!==e?e:Ce.box(e,t)}o(Ve,Pe);var Ee,Re,Ce=o(Ve,{box:function(e,t){\n/*ThouShaltNotCache*/\nvar n=Ae(t);return new Fe(e,Te(n),n.name,!0,n.equals)},array:function(e,t){\n/*ThouShaltNotCache*/\nvar n=Ae(t);return(!1===_t.useProxies||!1===n.proxy?er:gn)(e,Te(n),n.name)},map:function(e,t){\n/*ThouShaltNotCache*/\nvar n=Ae(t);return new Mn(e,Te(n),n.name)},set:function(e,t){\n/*ThouShaltNotCache*/\nvar n=Ae(t);return new En(e,Te(n),n.name)},object:function(e,t,n){\n/*ThouShaltNotCache*/\nreturn ir(function(){\n/*ThouShaltNotCache*/\nreturn Wt(!1===_t.useProxies||!1===(null==n?void 0:n.proxy)?Bn({},n):function(e,t){\n/*ThouShaltNotCache*/\nvar n,r;return d(),e=Bn(e,t),null!=(r=(n=e[H]).proxy_)?r:n.proxy_=new Proxy(e,on)}({},n),e,t)})},ref:G(je),shallow:G(xe),deep:Pe,struct:G(Me)}),De=\"computed\",Ne=ce(De),Le=ce(\"computed.struct\",{equals:X.structural}),Be=function(e,t){\n/*ThouShaltNotCache*/\nif(z(t))return Ne.decorate_20223_(e,t);if(g(t))return q(e,t,Ne);if(y(e))return G(ce(De,e));var n=y(t)?t:{};return n.get=e,n.name||(n.name=e.name||\"\"),new Xe(n)};Object.assign(Be,Ne),Be.struct=G(Le);var Ue=0,Ie=1,Ke=null!=(Ee=null==(Re=s(function(){},\"name\"))?void 0:Re.configurable)&&Ee,Ge={value:\"action\",configurable:!0,writable:!1,enumerable:!1};function qe(e,t,n,r){function i(){\n/*ThouShaltNotCache*/\nreturn ze(e,n,t,r||this,arguments)}\n/*ThouShaltNotCache*/\nreturn void 0===n&&(n=!1),i.isMobxAction=!0,i.toString=function(){\n/*ThouShaltNotCache*/\nreturn t.toString()},Ke&&(Ge.value=e,a(i,\"name\",Ge)),i}function ze(e,t,r,i,o){\n/*ThouShaltNotCache*/\nvar s=function(e,t){\n/*ThouShaltNotCache*/\nvar n=!1,r=0;0;var i=_t.trackingDerivation,o=!t||!i;bt();var s=_t.allowStateChanges;o&&(st(),s=We(!0));var a=ut(!0),u={runAsAction_:o,prevDerivation_:i,prevAllowStateChanges_:s,prevAllowStateReads_:a,notifySpy_:n,startTime_:r,actionId_:Ie++,parentActionId_:Ue};return Ue=u.actionId_,u}(0,t);try{return r.apply(i,o)}catch(e){throw s.error_=e,e}finally{!function(e){\n/*ThouShaltNotCache*/\nUe!==e.actionId_&&n(30);Ue=e.parentActionId_,void 0!==e.error_&&(_t.suppressReactionErrors=!0);$e(e.prevAllowStateChanges_),ct(e.prevAllowStateReads_),yt(),e.runAsAction_&&at(e.prevDerivation_);0;_t.suppressReactionErrors=!1}(s)}}function He(e,t){\n/*ThouShaltNotCache*/\nvar n=We(e);try{return t()}finally{$e(n)}}function We(e){\n/*ThouShaltNotCache*/\nvar t=_t.allowStateChanges;return _t.allowStateChanges=e,t}function $e(e){\n/*ThouShaltNotCache*/\n_t.allowStateChanges=e}var Fe=function(e){\n/*ThouShaltNotCache*/\nfunction t(t,n,r,i,o){\n/*ThouShaltNotCache*/\nvar s;return void 0===r&&(r=\"ObservableValue\"),void 0===i&&(i=!0),void 0===o&&(o=X.default),(s=e.call(this,r)||this).enhancer=void 0,s.name_=void 0,s.equals=void 0,s.hasUnreportedChange_=!1,s.interceptors_=void 0,s.changeListeners_=void 0,s.value_=void 0,s.dehancer=void 0,s.enhancer=n,s.name_=r,s.equals=o,s.value_=n(t,void 0,r),s}B(t,e);var n=t.prototype;return n.dehanceValue=function(e){\n/*ThouShaltNotCache*/\nreturn void 0!==this.dehancer?this.dehancer(e):e},n.set=function(e){\n/*ThouShaltNotCache*/\nthis.value_;if((e=this.prepareNewValue_(e))!==_t.UNCHANGED){0,this.setNewValue_(e)}},n.prepareNewValue_=function(e){if(\n/*ThouShaltNotCache*/\nnt(this),sn(this)){var t=un(this,{object:this,type:_n,newValue:e});if(!t)return _t.UNCHANGED;e=t.newValue}return e=this.enhancer(e,this.value_,this.name_),this.equals(this.value_,e)?_t.UNCHANGED:e},n.setNewValue_=function(e){\n/*ThouShaltNotCache*/\nvar t=this.value_;this.value_=e,this.reportChanged(),cn(this)&&hn(this,{type:_n,object:this,newValue:e,oldValue:t})},n.get=function(){\n/*ThouShaltNotCache*/\nreturn this.reportObserved(),this.dehanceValue(this.value_)},n.intercept_=function(e){\n/*ThouShaltNotCache*/\nreturn an(this,e)},n.observe_=function(e,t){\n/*ThouShaltNotCache*/\nreturn t&&e({observableKind:\"value\",debugObjectName:this.name_,object:this,type:_n,newValue:this.value_,oldValue:void 0}),ln(this,e)},n.raw=function(){\n/*ThouShaltNotCache*/\nreturn this.value_},n.toJSON=function(){\n/*ThouShaltNotCache*/\nreturn this.get()},n.toString=function(){\n/*ThouShaltNotCache*/\nreturn this.name_+\"[\"+this.value_+\"]\"},n.valueOf=function(){\n/*ThouShaltNotCache*/\nreturn M(this.get())},n[Symbol.toPrimitive]=function(){\n/*ThouShaltNotCache*/\nreturn this.valueOf()},t}(W),Xe=function(){\n/*ThouShaltNotCache*/\nfunction e(e){\n/*ThouShaltNotCache*/\nthis.dependenciesState_=Ye.NOT_TRACKING_,this.observing_=[],this.newObserving_=null,this.observers_=new Set,this.runId_=0,this.lastAccessedBy_=0,this.lowestObserverState_=Ye.UP_TO_DATE_,this.unboundDepsCount_=0,this.value_=new Ze(null),this.name_=void 0,this.triggeredBy_=void 0,this.flags_=0,this.derivation=void 0,this.setter_=void 0,this.isTracing_=Je.NONE,this.scope_=void 0,this.equals_=void 0,this.requiresReaction_=void 0,this.keepAlive_=void 0,this.onBOL=void 0,this.onBUOL=void 0,e.get||n(31),this.derivation=e.get,this.name_=e.name||\"ComputedValue\",e.set&&(this.setter_=qe(\"ComputedValue-setter\",e.set)),this.equals_=e.equals||(e.compareStructural||e.struct?X.structural:X.default),this.scope_=e.context,this.requiresReaction_=e.requiresReaction,this.keepAlive_=!!e.keepAlive}var t=e.prototype;return t.onBecomeStale_=function(){\n/*ThouShaltNotCache*/\n!function(e){\n/*ThouShaltNotCache*/\nif(e.lowestObserverState_!==Ye.UP_TO_DATE_)return;e.lowestObserverState_=Ye.POSSIBLY_STALE_,e.observers_.forEach(function(e){\n/*ThouShaltNotCache*/\ne.dependenciesState_===Ye.UP_TO_DATE_&&(e.dependenciesState_=Ye.POSSIBLY_STALE_,e.onBecomeStale_())})}(this)},t.onBO=function(){\n/*ThouShaltNotCache*/\nthis.onBOL&&this.onBOL.forEach(function(e){\n/*ThouShaltNotCache*/\nreturn e()})},t.onBUO=function(){\n/*ThouShaltNotCache*/\nthis.onBUOL&&this.onBUOL.forEach(function(e){\n/*ThouShaltNotCache*/\nreturn e()})},t.get=function(){if(\n/*ThouShaltNotCache*/\nthis.isComputing&&n(32,this.name_,this.derivation),0!==_t.inBatch||0!==this.observers_.size||this.keepAlive_){if(mt(this),tt(this)){var e=_t.trackingContext;this.keepAlive_&&!e&&(_t.trackingContext=this),this.trackAndCompute()&&function(e){\n/*ThouShaltNotCache*/\nif(e.lowestObserverState_===Ye.STALE_)return;e.lowestObserverState_=Ye.STALE_,e.observers_.forEach(function(t){\n/*ThouShaltNotCache*/\nt.dependenciesState_===Ye.POSSIBLY_STALE_?t.dependenciesState_=Ye.STALE_:t.dependenciesState_===Ye.UP_TO_DATE_&&(e.lowestObserverState_=Ye.UP_TO_DATE_)})}(this),_t.trackingContext=e}}else tt(this)&&(this.warnAboutUntrackedRead_(),bt(),this.value_=this.computeValue_(!1),yt());var t=this.value_;if(et(t))throw t.cause;return t},t.set=function(e){\n/*ThouShaltNotCache*/\nif(this.setter_){this.isRunningSetter&&n(33,this.name_),this.isRunningSetter=!0;try{this.setter_.call(this.scope_,e)}finally{this.isRunningSetter=!1}}else n(34,this.name_)},t.trackAndCompute=function(){\n/*ThouShaltNotCache*/\nvar e=this.value_,t=this.dependenciesState_===Ye.NOT_TRACKING_,n=this.computeValue_(!0),r=t||et(e)||et(n)||!this.equals_(e,n);return r&&(this.value_=n),r},t.computeValue_=function(e){\n/*ThouShaltNotCache*/\nthis.isComputing=!0;var t,n=We(!1);if(e)t=rt(this,this.derivation,this.scope_);else if(!0===_t.disableErrorBoundaries)t=this.derivation.call(this.scope_);else try{t=this.derivation.call(this.scope_)}catch(e){t=new Ze(e)}return $e(n),this.isComputing=!1,t},t.suspend_=function(){\n/*ThouShaltNotCache*/\nthis.keepAlive_||(it(this),this.value_=void 0)},t.observe_=function(e,t){\n/*ThouShaltNotCache*/\nvar n=this,r=!0,i=void 0;return Ut(function(){\n/*ThouShaltNotCache*/\nvar o=n.get();if(!r||t){var s=st();e({observableKind:\"computed\",debugObjectName:n.name_,type:_n,object:n,newValue:o,oldValue:i}),at(s)}r=!1,i=o})},t.warnAboutUntrackedRead_=function(){},t.toString=function(){\n/*ThouShaltNotCache*/\nreturn this.name_+\"[\"+this.derivation.toString()+\"]\"},t.valueOf=function(){\n/*ThouShaltNotCache*/\nreturn M(this.get())},t[Symbol.toPrimitive]=function(){\n/*ThouShaltNotCache*/\nreturn this.valueOf()},D(e,[{key:\"isComputing\",get:function(){\n/*ThouShaltNotCache*/\nreturn V(this.flags_,e.isComputingMask_)},set:function(t){\n/*ThouShaltNotCache*/\nthis.flags_=E(this.flags_,e.isComputingMask_,t)}},{key:\"isRunningSetter\",get:function(){\n/*ThouShaltNotCache*/\nreturn V(this.flags_,e.isRunningSetterMask_)},set:function(t){\n/*ThouShaltNotCache*/\nthis.flags_=E(this.flags_,e.isRunningSetterMask_,t)}},{key:\"isBeingObserved\",get:function(){\n/*ThouShaltNotCache*/\nreturn V(this.flags_,e.isBeingObservedMask_)},set:function(t){\n/*ThouShaltNotCache*/\nthis.flags_=E(this.flags_,e.isBeingObservedMask_,t)}},{key:\"isPendingUnobservation\",get:function(){\n/*ThouShaltNotCache*/\nreturn V(this.flags_,e.isPendingUnobservationMask_)},set:function(t){\n/*ThouShaltNotCache*/\nthis.flags_=E(this.flags_,e.isPendingUnobservationMask_,t)}},{key:\"diffValue\",get:function(){\n/*ThouShaltNotCache*/\nreturn V(this.flags_,e.diffValueMask_)?1:0},set:function(t){\n/*ThouShaltNotCache*/\nthis.flags_=E(this.flags_,e.diffValueMask_,1===t)}}])}();Xe.isComputingMask_=1,Xe.isRunningSetterMask_=2,Xe.isBeingObservedMask_=4,Xe.isPendingUnobservationMask_=8,Xe.diffValueMask_=16;var Ye,Je,Qe=S(\"ComputedValue\",Xe);!function(e){\n/*ThouShaltNotCache*/\ne[e.NOT_TRACKING_=-1]=\"NOT_TRACKING_\",e[e.UP_TO_DATE_=0]=\"UP_TO_DATE_\",e[e.POSSIBLY_STALE_=1]=\"POSSIBLY_STALE_\",e[e.STALE_=2]=\"STALE_\"}(Ye||(Ye={})),function(e){\n/*ThouShaltNotCache*/\ne[e.NONE=0]=\"NONE\",e[e.LOG=1]=\"LOG\",e[e.BREAK=2]=\"BREAK\"}(Je||(Je={}));var Ze=function(e){\n/*ThouShaltNotCache*/\nthis.cause=void 0,this.cause=e};function et(e){\n/*ThouShaltNotCache*/\nreturn e instanceof Ze}function tt(e){\n/*ThouShaltNotCache*/\nswitch(e.dependenciesState_){case Ye.UP_TO_DATE_:return!1;case Ye.NOT_TRACKING_:case Ye.STALE_:return!0;case Ye.POSSIBLY_STALE_:for(var t=ut(!0),n=st(),r=e.observing_,i=r.length,o=0;o<i;o++){var s=r[o];if(Qe(s)){if(_t.disableErrorBoundaries)s.get();else try{s.get()}catch(e){return at(n),ct(t),!0}if(e.dependenciesState_===Ye.STALE_)return at(n),ct(t),!0}}return lt(e),at(n),ct(t),!1}}function nt(e){}function rt(e,t,n){\n/*ThouShaltNotCache*/\nvar r=ut(!0);lt(e),e.newObserving_=new Array(0===e.runId_?100:e.observing_.length),e.unboundDepsCount_=0,e.runId_=++_t.runId;var i,o=_t.trackingDerivation;if(_t.trackingDerivation=e,_t.inBatch++,!0===_t.disableErrorBoundaries)i=t.call(n);else try{i=t.call(n)}catch(e){i=new Ze(e)}return _t.inBatch--,_t.trackingDerivation=o,function(e){for(\n/*ThouShaltNotCache*/\nvar t=e.observing_,n=e.observing_=e.newObserving_,r=Ye.UP_TO_DATE_,i=0,o=e.unboundDepsCount_,s=0;s<o;s++){var a=n[s];0===a.diffValue&&(a.diffValue=1,i!==s&&(n[i]=a),i++),a.dependenciesState_>r&&(r=a.dependenciesState_)}n.length=i,e.newObserving_=null,o=t.length;for(;o--;){var u=t[o];0===u.diffValue&&pt(u,e),u.diffValue=0}for(;i--;){var c=n[i];1===c.diffValue&&(c.diffValue=0,vt(c,e))}r!==Ye.UP_TO_DATE_&&(e.dependenciesState_=r,e.onBecomeStale_())}(e),ct(r),i}function it(e){\n/*ThouShaltNotCache*/\nvar t=e.observing_;e.observing_=[];for(var n=t.length;n--;)pt(t[n],e);e.dependenciesState_=Ye.NOT_TRACKING_}function ot(e){\n/*ThouShaltNotCache*/\nvar t=st();try{return e()}finally{at(t)}}function st(){\n/*ThouShaltNotCache*/\nvar e=_t.trackingDerivation;return _t.trackingDerivation=null,e}function at(e){\n/*ThouShaltNotCache*/\n_t.trackingDerivation=e}function ut(e){\n/*ThouShaltNotCache*/\nvar t=_t.allowStateReads;return _t.allowStateReads=e,t}function ct(e){\n/*ThouShaltNotCache*/\n_t.allowStateReads=e}function lt(e){\n/*ThouShaltNotCache*/\nif(e.dependenciesState_!==Ye.UP_TO_DATE_){e.dependenciesState_=Ye.UP_TO_DATE_;for(var t=e.observing_,n=t.length;n--;)t[n].lowestObserverState_=Ye.UP_TO_DATE_}}var ht=function(){\n/*ThouShaltNotCache*/\nthis.version=6,this.UNCHANGED={},this.trackingDerivation=null,this.trackingContext=null,this.runId=0,this.mobxGuid=0,this.inBatch=0,this.pendingUnobservations=[],this.pendingReactions=[],this.isRunningReactions=!1,this.allowStateChanges=!1,this.allowStateReads=!0,this.enforceActions=!0,this.spyListeners=[],this.globalReactionErrorHandlers=[],this.computedRequiresReaction=!1,this.reactionRequiresObservable=!1,this.observableRequiresReaction=!1,this.disableErrorBoundaries=!1,this.suppressReactionErrors=!1,this.useProxies=!0,this.verifyProxies=!1,this.safeDescriptors=!0},ft=!0,dt=!1,_t=function(){\n/*ThouShaltNotCache*/\nvar e=i();return e.__mobxInstanceCount>0&&!e.__mobxGlobals&&(ft=!1),e.__mobxGlobals&&e.__mobxGlobals.version!==(new ht).version&&(ft=!1),ft?e.__mobxGlobals?(e.__mobxInstanceCount+=1,e.__mobxGlobals.UNCHANGED||(e.__mobxGlobals.UNCHANGED={}),e.__mobxGlobals):(e.__mobxInstanceCount=1,e.__mobxGlobals=new ht):(setTimeout(function(){\n/*ThouShaltNotCache*/\ndt||n(35)},1),new ht)}();function vt(e,t){\n/*ThouShaltNotCache*/\ne.observers_.add(t),e.lowestObserverState_>t.dependenciesState_&&(e.lowestObserverState_=t.dependenciesState_)}function pt(e,t){\n/*ThouShaltNotCache*/\ne.observers_.delete(t),0===e.observers_.size&&gt(e)}function gt(e){\n/*ThouShaltNotCache*/\n!1===e.isPendingUnobservation&&(e.isPendingUnobservation=!0,_t.pendingUnobservations.push(e))}function bt(){\n/*ThouShaltNotCache*/\n_t.inBatch++}function yt(){\n/*ThouShaltNotCache*/\nif(0===--_t.inBatch){kt();for(var e=_t.pendingUnobservations,t=0;t<e.length;t++){var n=e[t];n.isPendingUnobservation=!1,0===n.observers_.size&&(n.isBeingObserved&&(n.isBeingObserved=!1,n.onBUO()),n instanceof Xe&&n.suspend_())}_t.pendingUnobservations=[]}}function mt(e){var t=_t.trackingDerivation;return null!==t?(t.runId_!==e.lastAccessedBy_&&(e.lastAccessedBy_=t.runId_,t.newObserving_[t.unboundDepsCount_++]=e,!e.isBeingObserved&&_t.trackingContext&&(e.isBeingObserved=!0,e.onBO())),e.isBeingObserved):(0===e.observers_.size&&_t.inBatch>0&&gt(e),!1)}function Ot(e){\n/*ThouShaltNotCache*/\ne.lowestObserverState_!==Ye.STALE_&&(e.lowestObserverState_=Ye.STALE_,e.observers_.forEach(function(e){\n/*ThouShaltNotCache*/\ne.dependenciesState_===Ye.UP_TO_DATE_&&e.onBecomeStale_(),e.dependenciesState_=Ye.STALE_}))}var wt=function(){\n/*ThouShaltNotCache*/\nfunction e(e,t,n,r){\n/*ThouShaltNotCache*/\nvoid 0===e&&(e=\"Reaction\"),this.name_=void 0,this.onInvalidate_=void 0,this.errorHandler_=void 0,this.requiresObservable_=void 0,this.observing_=[],this.newObserving_=[],this.dependenciesState_=Ye.NOT_TRACKING_,this.runId_=0,this.unboundDepsCount_=0,this.flags_=0,this.isTracing_=Je.NONE,this.name_=e,this.onInvalidate_=t,this.errorHandler_=n,this.requiresObservable_=r}var t=e.prototype;return t.onBecomeStale_=function(){\n/*ThouShaltNotCache*/\nthis.schedule_()},t.schedule_=function(){\n/*ThouShaltNotCache*/\nthis.isScheduled||(this.isScheduled=!0,_t.pendingReactions.push(this),kt())},t.runReaction_=function(){\n/*ThouShaltNotCache*/\nif(!this.isDisposed){bt(),this.isScheduled=!1;var e=_t.trackingContext;if(_t.trackingContext=this,tt(this)){this.isTrackPending=!0;try{this.onInvalidate_()}catch(e){this.reportExceptionInDerivation_(e)}}_t.trackingContext=e,yt()}},t.track=function(e){\n/*ThouShaltNotCache*/\nif(!this.isDisposed){bt();0,this.isRunning=!0;var t=_t.trackingContext;_t.trackingContext=this;var n=rt(this,e,void 0);_t.trackingContext=t,this.isRunning=!1,this.isTrackPending=!1,this.isDisposed&&it(this),et(n)&&this.reportExceptionInDerivation_(n.cause),yt()}},t.reportExceptionInDerivation_=function(e){\n/*ThouShaltNotCache*/\nvar t=this;if(this.errorHandler_)this.errorHandler_(e,this);else{if(_t.disableErrorBoundaries)throw e;var n=\"[mobx] uncaught error in '\"+this+\"'\";_t.suppressReactionErrors||console.error(n,e),_t.globalReactionErrorHandlers.forEach(function(n){\n/*ThouShaltNotCache*/\nreturn n(e,t)})}},t.dispose=function(){\n/*ThouShaltNotCache*/\nthis.isDisposed||(this.isDisposed=!0,this.isRunning||(bt(),it(this),yt()))},t.getDisposer_=function(e){\n/*ThouShaltNotCache*/\nvar t=this,n=function n(){\n/*ThouShaltNotCache*/\nt.dispose(),null==e||null==e.removeEventListener||e.removeEventListener(\"abort\",n)};return null==e||null==e.addEventListener||e.addEventListener(\"abort\",n),n[H]=this,n},t.toString=function(){\n/*ThouShaltNotCache*/\nreturn\"Reaction[\"+this.name_+\"]\"},t.trace=function(e){\n/*ThouShaltNotCache*/\nvoid 0===e&&(e=!1)},D(e,[{key:\"isDisposed\",get:function(){\n/*ThouShaltNotCache*/\nreturn V(this.flags_,e.isDisposedMask_)},set:function(t){\n/*ThouShaltNotCache*/\nthis.flags_=E(this.flags_,e.isDisposedMask_,t)}},{key:\"isScheduled\",get:function(){\n/*ThouShaltNotCache*/\nreturn V(this.flags_,e.isScheduledMask_)},set:function(t){\n/*ThouShaltNotCache*/\nthis.flags_=E(this.flags_,e.isScheduledMask_,t)}},{key:\"isTrackPending\",get:function(){\n/*ThouShaltNotCache*/\nreturn V(this.flags_,e.isTrackPendingMask_)},set:function(t){\n/*ThouShaltNotCache*/\nthis.flags_=E(this.flags_,e.isTrackPendingMask_,t)}},{key:\"isRunning\",get:function(){\n/*ThouShaltNotCache*/\nreturn V(this.flags_,e.isRunningMask_)},set:function(t){\n/*ThouShaltNotCache*/\nthis.flags_=E(this.flags_,e.isRunningMask_,t)}},{key:\"diffValue\",get:function(){\n/*ThouShaltNotCache*/\nreturn V(this.flags_,e.diffValueMask_)?1:0},set:function(t){\n/*ThouShaltNotCache*/\nthis.flags_=E(this.flags_,e.diffValueMask_,1===t)}}])}();wt.isDisposedMask_=1,wt.isScheduledMask_=2,wt.isTrackPendingMask_=4,wt.isRunningMask_=8,wt.diffValueMask_=16;var St=100,At=function(e){\n/*ThouShaltNotCache*/\nreturn e()};function kt(){\n/*ThouShaltNotCache*/\n_t.inBatch>0||_t.isRunningReactions||At(jt)}function jt(){\n/*ThouShaltNotCache*/\n_t.isRunningReactions=!0;for(var e=_t.pendingReactions,t=0;e.length>0;){++t===St&&(console.error(\"[mobx] cycle in reaction: \"+e[0]),e.splice(0));for(var n=e.splice(0),r=0,i=n.length;r<i;r++)n[r].runReaction_()}_t.isRunningReactions=!1}var xt=S(\"Reaction\",wt);var Mt=\"action\",Pt=\"autoAction\",Tt=\"<unnamed action>\",Vt=Z(Mt),Et=Z(\"action.bound\",{bound:!0}),Rt=Z(Pt,{autoAction:!0}),Ct=Z(\"autoAction.bound\",{autoAction:!0,bound:!0});function Dt(e){return function(t,n){\n/*ThouShaltNotCache*/\nreturn p(t)?qe(t.name||Tt,t,e):p(n)?qe(t,n,e):z(n)?(e?Rt:Vt).decorate_20223_(t,n):g(n)?q(t,n,e?Rt:Vt):g(t)?G(Z(e?Pt:Mt,{name:t,autoAction:e})):void 0}}var Nt=Dt(!1);Object.assign(Nt,Vt);var Lt=Dt(!0);function Bt(e){\n/*ThouShaltNotCache*/\nreturn p(e)&&!0===e.isMobxAction}function Ut(e,t){\n/*ThouShaltNotCache*/\nvar n,r,i,o;void 0===t&&(t=l);var s,a=null!=(n=null==(r=t)?void 0:r.name)?n:\"Autorun\";if(!t.scheduler&&!t.delay)s=new wt(a,function(){\n/*ThouShaltNotCache*/\nthis.track(h)},t.onError,t.requiresObservable);else{var u=Kt(t),c=!1;s=new wt(a,function(){\n/*ThouShaltNotCache*/\nc||(c=!0,u(function(){\n/*ThouShaltNotCache*/\nc=!1,s.isDisposed||s.track(h)}))},t.onError,t.requiresObservable)}function h(){\n/*ThouShaltNotCache*/\ne(s)}return null!=(i=t)&&null!=(i=i.signal)&&i.aborted||s.schedule_(),s.getDisposer_(null==(o=t)?void 0:o.signal)}Object.assign(Lt,Rt),Nt.bound=G(Et),Lt.bound=G(Ct);var It=function(e){\n/*ThouShaltNotCache*/\nreturn e()};function Kt(e){\n/*ThouShaltNotCache*/\nreturn e.scheduler?e.scheduler:e.delay?function(t){\n/*ThouShaltNotCache*/\nreturn setTimeout(t,e.delay)}:It}var Gt=\"onBO\",qt=\"onBUO\";function zt(e,t,n){\n/*ThouShaltNotCache*/\nreturn Ht(qt,e,t,n)}function Ht(e,t,n,r){\n/*ThouShaltNotCache*/\nvar i=\"function\"==typeof r?tr(t,n):tr(t),o=p(r)?r:n,s=e+\"L\";return i[s]?i[s].add(o):i[s]=new Set([o]),function(){\n/*ThouShaltNotCache*/\nvar e=i[s];e&&(e.delete(o),0===e.size&&delete i[s])}}function Wt(e,t,n,r){var i=T(t);return ir(function(){\n/*ThouShaltNotCache*/\nvar t=Bn(e,r)[H];x(i).forEach(function(e){\n/*ThouShaltNotCache*/\nt.extend_(e,i[e],!n||(!(e in n)||n[e]))})}),e}var $t=0;function Ft(){\n/*ThouShaltNotCache*/\nthis.message=\"FLOW_CANCELLED\"}Ft.prototype=Object.create(Error.prototype);var Xt=ie(\"flow\"),Yt=ie(\"flow.bound\",{bound:!0}),Jt=Object.assign(function(e,t){\n/*ThouShaltNotCache*/\nif(z(t))return Xt.decorate_20223_(e,t);if(g(t))return q(e,t,Xt);var n=e,r=n.name||\"<unnamed flow>\",i=function(){\n/*ThouShaltNotCache*/\nvar e,t=arguments,i=++$t,o=Nt(r+\" - runid: \"+i+\" - init\",n).apply(this,t),s=void 0,a=new Promise(function(t,n){\n/*ThouShaltNotCache*/\nvar a=0;function u(e){var t;\n/*ThouShaltNotCache*/\ns=void 0;try{t=Nt(r+\" - runid: \"+i+\" - yield \"+a++,o.next).call(o,e)}catch(e){return n(e)}l(t)}function c(e){var t;\n/*ThouShaltNotCache*/\ns=void 0;try{t=Nt(r+\" - runid: \"+i+\" - yield \"+a++,o.throw).call(o,e)}catch(e){return n(e)}l(t)}function l(e){\n/*ThouShaltNotCache*/\nif(!p(null==e?void 0:e.then))return e.done?t(e.value):(s=Promise.resolve(e.value)).then(u,c);e.then(l,n)}e=n,u(void 0)});return a.cancel=Nt(r+\" - runid: \"+i+\" - cancel\",function(){\n/*ThouShaltNotCache*/\ntry{s&&Qt(s);var t=o.return(void 0),n=Promise.resolve(t.value);n.then(v,v),Qt(n),e(new Ft)}catch(t){e(t)}}),a};return i.isMobXFlow=!0,i},Xt);function Qt(e){\n/*ThouShaltNotCache*/\np(e.cancel)&&e.cancel()}function Zt(e){\n/*ThouShaltNotCache*/\nreturn!0===(null==e?void 0:e.isMobXFlow)}function en(e,t){\n/*ThouShaltNotCache*/\nreturn!!e&&(void 0!==t?!!Kn(e)&&e[H].values_.has(t):Kn(e)||!!e[H]||$(e)||xt(e)||Qe(e))}function tn(e){return en(e)}function nn(e,t){\n/*ThouShaltNotCache*/\nvoid 0===t&&(t=void 0),bt();try{return e.apply(t)}finally{yt()}}function rn(e){\n/*ThouShaltNotCache*/\nreturn e[H]}Jt.bound=G(Yt);var on={has:function(e,t){return rn(e).has_(t)},get:function(e,t){\n/*ThouShaltNotCache*/\nreturn rn(e).get_(t)},set:function(e,t,n){\n/*ThouShaltNotCache*/\nvar r;return!!g(t)&&(null==(r=rn(e).set_(t,n,!0))||r)},deleteProperty:function(e,t){\n/*ThouShaltNotCache*/\nvar n;return!!g(t)&&(null==(n=rn(e).delete_(t,!0))||n)},defineProperty:function(e,t,n){\n/*ThouShaltNotCache*/\nvar r;return null==(r=rn(e).defineProperty_(t,n))||r},ownKeys:function(e){return rn(e).ownKeys_()},preventExtensions:function(e){\n/*ThouShaltNotCache*/\nn(13)}};function sn(e){\n/*ThouShaltNotCache*/\nreturn void 0!==e.interceptors_&&e.interceptors_.length>0}function an(e,t){\n/*ThouShaltNotCache*/\nvar n=e.interceptors_||(e.interceptors_=[]);return n.push(t),_(function(){\n/*ThouShaltNotCache*/\nvar e=n.indexOf(t);-1!==e&&n.splice(e,1)})}function un(e,t){\n/*ThouShaltNotCache*/\nvar r=st();try{for(var i=[].concat(e.interceptors_||[]),o=0,s=i.length;o<s&&((t=i[o](t))&&!t.type&&n(14),t);o++);return t}finally{at(r)}}function cn(e){\n/*ThouShaltNotCache*/\nreturn void 0!==e.changeListeners_&&e.changeListeners_.length>0}function ln(e,t){\n/*ThouShaltNotCache*/\nvar n=e.changeListeners_||(e.changeListeners_=[]);return n.push(t),_(function(){\n/*ThouShaltNotCache*/\nvar e=n.indexOf(t);-1!==e&&n.splice(e,1)})}function hn(e,t){\n/*ThouShaltNotCache*/\nvar n=st(),r=e.changeListeners_;if(r){for(var i=0,o=(r=r.slice()).length;i<o;i++)r[i](t);at(n)}}function fn(e,t,n){\n/*ThouShaltNotCache*/\nreturn ir(function(){\n/*ThouShaltNotCache*/\nvar r=Bn(e,n)[H];null!=t||(t=function(e){\n/*ThouShaltNotCache*/\nreturn P(e,K)||O(e,K,L({},e[K])),e[K]}(e)),x(t).forEach(function(e){\n/*ThouShaltNotCache*/\nreturn r.make_(e,t[e])})}),e}var dn=\"splice\",_n=\"update\",vn={get:function(e,t){\n/*ThouShaltNotCache*/\nvar n=e[H];return t===H?n:\"length\"===t?n.getArrayLength_():\"string\"!=typeof t||isNaN(t)?P(bn,t)?bn[t]:e[t]:n.get_(parseInt(t))},set:function(e,t,n){\n/*ThouShaltNotCache*/\nvar r=e[H];return\"length\"===t&&r.setArrayLength_(n),\"symbol\"==typeof t||isNaN(t)?e[t]=n:r.set_(parseInt(t),n),!0},preventExtensions:function(){\n/*ThouShaltNotCache*/\nn(15)}},pn=function(){\n/*ThouShaltNotCache*/\nfunction e(e,t,n,r){\n/*ThouShaltNotCache*/\nvoid 0===e&&(e=\"ObservableArray\"),this.owned_=void 0,this.legacyMode_=void 0,this.atom_=void 0,this.values_=[],this.interceptors_=void 0,this.changeListeners_=void 0,this.enhancer_=void 0,this.dehancer=void 0,this.proxy_=void 0,this.lastKnownLength_=0,this.owned_=n,this.legacyMode_=r,this.atom_=new W(e),this.enhancer_=function(e,n){\n/*ThouShaltNotCache*/\nreturn t(e,n,\"ObservableArray[..]\")}}var t=e.prototype;return t.dehanceValue_=function(e){\n/*ThouShaltNotCache*/\nreturn void 0!==this.dehancer?this.dehancer(e):e},t.dehanceValues_=function(e){\n/*ThouShaltNotCache*/\nreturn void 0!==this.dehancer&&e.length>0?e.map(this.dehancer):e},t.intercept_=function(e){\n/*ThouShaltNotCache*/\nreturn an(this,e)},t.observe_=function(e,t){\n/*ThouShaltNotCache*/\nreturn void 0===t&&(t=!1),t&&e({observableKind:\"array\",object:this.proxy_,debugObjectName:this.atom_.name_,type:\"splice\",index:0,added:this.values_.slice(),addedCount:this.values_.length,removed:[],removedCount:0}),ln(this,e)},t.getArrayLength_=function(){\n/*ThouShaltNotCache*/\nreturn this.atom_.reportObserved(),this.values_.length},t.setArrayLength_=function(e){\n/*ThouShaltNotCache*/\n(\"number\"!=typeof e||isNaN(e)||e<0)&&n(\"Out of range: \"+e);var t=this.values_.length;if(e!==t)if(e>t){for(var r=new Array(e-t),i=0;i<e-t;i++)r[i]=void 0;this.spliceWithArray_(t,0,r)}else this.spliceWithArray_(e,t-e)},t.updateArrayLength_=function(e,t){\n/*ThouShaltNotCache*/\ne!==this.lastKnownLength_&&n(16),this.lastKnownLength_+=t,this.legacyMode_&&t>0&&Zn(e+t+1)},t.spliceWithArray_=function(e,t,n){\n/*ThouShaltNotCache*/\nvar r=this;this.atom_;var i=this.values_.length;if(void 0===e?e=0:e>i?e=i:e<0&&(e=Math.max(0,i+e)),t=1===arguments.length?i-e:null==t?0:Math.max(0,Math.min(t,i-e)),void 0===n&&(n=c),sn(this)){var o=un(this,{object:this.proxy_,type:dn,index:e,removedCount:t,added:n});if(!o)return c;t=o.removedCount,n=o.added}if(n=0===n.length?n:n.map(function(e){\n/*ThouShaltNotCache*/\nreturn r.enhancer_(e,void 0)}),this.legacyMode_){var s=n.length-t;this.updateArrayLength_(i,s)}var a=this.spliceItemsIntoValues_(e,t,n);return 0===t&&0===n.length||this.notifyArraySplice_(e,n,a),this.dehanceValues_(a)},t.spliceItemsIntoValues_=function(e,t,n){var r;\n/*ThouShaltNotCache*/\nif(n.length<1e4)return(r=this.values_).splice.apply(r,[e,t].concat(n));var i=this.values_.slice(e,e+t),o=this.values_.slice(e+t);this.values_.length+=n.length-t;for(var s=0;s<n.length;s++)this.values_[e+s]=n[s];for(var a=0;a<o.length;a++)this.values_[e+n.length+a]=o[a];return i},t.notifyArrayChildUpdate_=function(e,t,n){\n/*ThouShaltNotCache*/\nvar r=!this.owned_&&!1,i=cn(this),o=i||r?{observableKind:\"array\",object:this.proxy_,type:_n,debugObjectName:this.atom_.name_,index:e,newValue:t,oldValue:n}:null;this.atom_.reportChanged(),i&&hn(this,o)},t.notifyArraySplice_=function(e,t,n){\n/*ThouShaltNotCache*/\nvar r=!this.owned_&&!1,i=cn(this),o=i||r?{observableKind:\"array\",object:this.proxy_,debugObjectName:this.atom_.name_,type:dn,index:e,removed:n,added:t,removedCount:n.length,addedCount:t.length}:null;this.atom_.reportChanged(),i&&hn(this,o)},t.get_=function(e){\n/*ThouShaltNotCache*/\nif(!(this.legacyMode_&&e>=this.values_.length))return this.atom_.reportObserved(),this.dehanceValue_(this.values_[e]);console.warn(\"[mobx] Out of bounds read: \"+e)},t.set_=function(e,t){\n/*ThouShaltNotCache*/\nvar r=this.values_;if(this.legacyMode_&&e>r.length&&n(17,e,r.length),e<r.length){this.atom_;var i=r[e];if(sn(this)){var o=un(this,{type:_n,object:this.proxy_,index:e,newValue:t});if(!o)return;t=o.newValue}(t=this.enhancer_(t,i))!==i&&(r[e]=t,this.notifyArrayChildUpdate_(e,t,i))}else{for(var s=new Array(e+1-r.length),a=0;a<s.length-1;a++)s[a]=void 0;s[s.length-1]=t,this.spliceWithArray_(r.length,0,s)}},e}();function gn(e,t,n,r){\n/*ThouShaltNotCache*/\nreturn void 0===n&&(n=\"ObservableArray\"),void 0===r&&(r=!1),d(),ir(function(){\n/*ThouShaltNotCache*/\nvar i=new pn(n,t,r,!1);w(i.values_,H,i);var o=new Proxy(i.values_,vn);return i.proxy_=o,e&&e.length&&i.spliceWithArray_(0,0,e),o})}var bn={clear:function(){\n/*ThouShaltNotCache*/\nreturn this.splice(0)},replace:function(e){\n/*ThouShaltNotCache*/\nvar t=this[H];return t.spliceWithArray_(0,t.values_.length,e)},toJSON:function(){\n/*ThouShaltNotCache*/\nreturn this.slice()},splice:function(e,t){\n/*ThouShaltNotCache*/\nfor(var n=arguments.length,r=new Array(n>2?n-2:0),i=2;i<n;i++)r[i-2]=arguments[i];var o=this[H];switch(arguments.length){case 0:return[];case 1:return o.spliceWithArray_(e);case 2:return o.spliceWithArray_(e,t)}return o.spliceWithArray_(e,t,r)},spliceWithArray:function(e,t,n){\n/*ThouShaltNotCache*/\nreturn this[H].spliceWithArray_(e,t,n)},push:function(){for(\n/*ThouShaltNotCache*/\nvar e=this[H],t=arguments.length,n=new Array(t),r=0;r<t;r++)n[r]=arguments[r];return e.spliceWithArray_(e.values_.length,0,n),e.values_.length},pop:function(){\n/*ThouShaltNotCache*/\nreturn this.splice(Math.max(this[H].values_.length-1,0),1)[0]},shift:function(){\n/*ThouShaltNotCache*/\nreturn this.splice(0,1)[0]},unshift:function(){for(\n/*ThouShaltNotCache*/\nvar e=this[H],t=arguments.length,n=new Array(t),r=0;r<t;r++)n[r]=arguments[r];return e.spliceWithArray_(0,0,n),e.values_.length},reverse:function(){\n/*ThouShaltNotCache*/\nreturn _t.trackingDerivation&&n(37,\"reverse\"),this.replace(this.slice().reverse()),this},sort:function(){\n/*ThouShaltNotCache*/\n_t.trackingDerivation&&n(37,\"sort\");var e=this.slice();return e.sort.apply(e,arguments),this.replace(e),this},remove:function(e){\n/*ThouShaltNotCache*/\nvar t=this[H],n=t.dehanceValues_(t.values_).indexOf(e);return n>-1&&(this.splice(n,1),!0)}};function yn(e,t){\n/*ThouShaltNotCache*/\n\"function\"==typeof Array.prototype[e]&&(bn[e]=t(e))}function mn(e){\n/*ThouShaltNotCache*/\nreturn function(){\n/*ThouShaltNotCache*/\nvar t=this[H];t.atom_.reportObserved();var n=t.dehanceValues_(t.values_);return n[e].apply(n,arguments)}}function On(e){\n/*ThouShaltNotCache*/\nreturn function(t,n){\n/*ThouShaltNotCache*/\nvar r=this,i=this[H];return i.atom_.reportObserved(),i.dehanceValues_(i.values_)[e](function(e,i){\n/*ThouShaltNotCache*/\nreturn t.call(n,e,i,r)})}}function wn(e){\n/*ThouShaltNotCache*/\nreturn function(){\n/*ThouShaltNotCache*/\nvar t=this,n=this[H];n.atom_.reportObserved();var r=n.dehanceValues_(n.values_),i=arguments[0];return arguments[0]=function(e,n,r){\n/*ThouShaltNotCache*/\nreturn i(e,n,r,t)},r[e].apply(r,arguments)}}yn(\"at\",mn),yn(\"concat\",mn),yn(\"flat\",mn),yn(\"includes\",mn),yn(\"indexOf\",mn),yn(\"join\",mn),yn(\"lastIndexOf\",mn),yn(\"slice\",mn),yn(\"toString\",mn),yn(\"toLocaleString\",mn),yn(\"toSorted\",mn),yn(\"toSpliced\",mn),yn(\"with\",mn),yn(\"every\",On),yn(\"filter\",On),yn(\"find\",On),yn(\"findIndex\",On),yn(\"findLast\",On),yn(\"findLastIndex\",On),yn(\"flatMap\",On),yn(\"forEach\",On),yn(\"map\",On),yn(\"some\",On),yn(\"toReversed\",On),yn(\"reduce\",wn),yn(\"reduceRight\",wn);var Sn=S(\"ObservableArrayAdministration\",pn);function An(e){\n/*ThouShaltNotCache*/\nreturn b(e)&&Sn(e[H])}var kn={},jn=\"add\",xn=\"delete\",Mn=function(){\n/*ThouShaltNotCache*/\nfunction e(e,t,r){\n/*ThouShaltNotCache*/\nvar i=this;void 0===t&&(t=Y),void 0===r&&(r=\"ObservableMap\"),this.enhancer_=void 0,this.name_=void 0,this[H]=kn,this.data_=void 0,this.hasMap_=void 0,this.keysAtom_=void 0,this.interceptors_=void 0,this.changeListeners_=void 0,this.dehancer=void 0,this.enhancer_=t,this.name_=r,p(Map)||n(18),ir(function(){\n/*ThouShaltNotCache*/\ni.keysAtom_=F(\"ObservableMap.keys()\"),i.data_=new Map,i.hasMap_=new Map,e&&i.merge(e)})}var t=e.prototype;return t.has_=function(e){\n/*ThouShaltNotCache*/\nreturn this.data_.has(e)},t.has=function(e){\n/*ThouShaltNotCache*/\nvar t=this;if(!_t.trackingDerivation)return this.has_(e);var n=this.hasMap_.get(e);if(!n){var r=n=new Fe(this.has_(e),J,\"ObservableMap.key?\",!1);this.hasMap_.set(e,r),zt(r,function(){\n/*ThouShaltNotCache*/\nreturn t.hasMap_.delete(e)})}return n.get()},t.set=function(e,t){\n/*ThouShaltNotCache*/\nvar n=this.has_(e);if(sn(this)){var r=un(this,{type:n?_n:jn,object:this,newValue:t,name:e});if(!r)return this;t=r.newValue}return n?this.updateValue_(e,t):this.addValue_(e,t),this},t.delete=function(e){\n/*ThouShaltNotCache*/\nvar t=this;if((this.keysAtom_,sn(this))&&!un(this,{type:xn,object:this,name:e}))return!1;if(this.has_(e)){var n=cn(this),r=n?{observableKind:\"map\",debugObjectName:this.name_,type:xn,object:this,oldValue:this.data_.get(e).value_,name:e}:null;return nn(function(){\n/*ThouShaltNotCache*/\nvar n;t.keysAtom_.reportChanged(),null==(n=t.hasMap_.get(e))||n.setNewValue_(!1),t.data_.get(e).setNewValue_(void 0),t.data_.delete(e)}),n&&hn(this,r),!0}return!1},t.updateValue_=function(e,t){\n/*ThouShaltNotCache*/\nvar n=this.data_.get(e);if((t=n.prepareNewValue_(t))!==_t.UNCHANGED){var r=cn(this),i=r?{observableKind:\"map\",debugObjectName:this.name_,type:_n,object:this,oldValue:n.value_,name:e,newValue:t}:null;0,n.setNewValue_(t),r&&hn(this,i)}},t.addValue_=function(e,t){\n/*ThouShaltNotCache*/\nvar n=this;this.keysAtom_,nn(function(){\n/*ThouShaltNotCache*/\nvar r,i=new Fe(t,n.enhancer_,\"ObservableMap.key\",!1);n.data_.set(e,i),t=i.value_,null==(r=n.hasMap_.get(e))||r.setNewValue_(!0),n.keysAtom_.reportChanged()});var r=cn(this),i=r?{observableKind:\"map\",debugObjectName:this.name_,type:jn,object:this,name:e,newValue:t}:null;r&&hn(this,i)},t.get=function(e){\n/*ThouShaltNotCache*/\nreturn this.has(e)?this.dehanceValue_(this.data_.get(e).get()):this.dehanceValue_(void 0)},t.dehanceValue_=function(e){\n/*ThouShaltNotCache*/\nreturn void 0!==this.dehancer?this.dehancer(e):e},t.keys=function(){\n/*ThouShaltNotCache*/\nreturn this.keysAtom_.reportObserved(),this.data_.keys()},t.values=function(){\n/*ThouShaltNotCache*/\nvar e=this,t=this.keys();return Tn({next:function(){\n/*ThouShaltNotCache*/\nvar n=t.next(),r=n.done,i=n.value;return{done:r,value:r?void 0:e.get(i)}}})},t.entries=function(){\n/*ThouShaltNotCache*/\nvar e=this,t=this.keys();return Tn({next:function(){\n/*ThouShaltNotCache*/\nvar n=t.next(),r=n.done,i=n.value;return{done:r,value:r?void 0:[i,e.get(i)]}}})},t[Symbol.iterator]=function(){\n/*ThouShaltNotCache*/\nreturn this.entries()},t.forEach=function(e,t){\n/*ThouShaltNotCache*/\nfor(var n,r=N(this);!(n=r()).done;){var i=n.value,o=i[0],s=i[1];e.call(t,s,o,this)}},t.merge=function(e){\n/*ThouShaltNotCache*/\nvar t=this;return Pn(e)&&(e=new Map(e)),nn(function(){var r,i,o;\n/*ThouShaltNotCache*/\ny(e)?function(e){\n/*ThouShaltNotCache*/\nvar t=Object.keys(e);if(!j)return t;var n=Object.getOwnPropertySymbols(e);return n.length?[].concat(t,n.filter(function(t){\n/*ThouShaltNotCache*/\nreturn u.propertyIsEnumerable.call(e,t)})):t}(e).forEach(function(n){\n/*ThouShaltNotCache*/\nreturn t.set(n,e[n])}):Array.isArray(e)?e.forEach(function(e){\n/*ThouShaltNotCache*/\nvar n=e[0],r=e[1];return t.set(n,r)}):A(e)?(r=e,i=Object.getPrototypeOf(r),o=Object.getPrototypeOf(i),null!==Object.getPrototypeOf(o)&&n(19,e),e.forEach(function(e,n){\n/*ThouShaltNotCache*/\nreturn t.set(n,e)})):null!=e&&n(20,e)}),this},t.clear=function(){\n/*ThouShaltNotCache*/\nvar e=this;nn(function(){\n/*ThouShaltNotCache*/\not(function(){\n/*ThouShaltNotCache*/\nfor(var t,n=N(e.keys());!(t=n()).done;){var r=t.value;e.delete(r)}})})},t.replace=function(e){\n/*ThouShaltNotCache*/\nvar t=this;return nn(function(){for(\n/*ThouShaltNotCache*/\nvar r,i=function(e){\n/*ThouShaltNotCache*/\nif(A(e)||Pn(e))return e;if(Array.isArray(e))return new Map(e);if(y(e)){var t=new Map;for(var r in e)t.set(r,e[r]);return t}return n(21,e)}(e),o=new Map,s=!1,a=N(t.data_.keys());!(r=a()).done;){var u=r.value;if(!i.has(u))if(t.delete(u))s=!0;else{var c=t.data_.get(u);o.set(u,c)}}for(var l,h=N(i.entries());!(l=h()).done;){var f=l.value,d=f[0],_=f[1],v=t.data_.has(d);if(t.set(d,_),t.data_.has(d)){var p=t.data_.get(d);o.set(d,p),v||(s=!0)}}if(!s)if(t.data_.size!==o.size)t.keysAtom_.reportChanged();else for(var g=t.data_.keys(),b=o.keys(),m=g.next(),O=b.next();!m.done;){if(m.value!==O.value){t.keysAtom_.reportChanged();break}m=g.next(),O=b.next()}t.data_=o}),this},t.toString=function(){\n/*ThouShaltNotCache*/\nreturn\"[object ObservableMap]\"},t.toJSON=function(){\n/*ThouShaltNotCache*/\nreturn Array.from(this)},t.observe_=function(e,t){return ln(this,e)},t.intercept_=function(e){\n/*ThouShaltNotCache*/\nreturn an(this,e)},D(e,[{key:\"size\",get:function(){\n/*ThouShaltNotCache*/\nreturn this.keysAtom_.reportObserved(),this.data_.size}},{key:Symbol.toStringTag,get:function(){\n/*ThouShaltNotCache*/\nreturn\"Map\"}}])}(),Pn=S(\"ObservableMap\",Mn);function Tn(e){\n/*ThouShaltNotCache*/\nreturn e[Symbol.toStringTag]=\"MapIterator\",hr(e)}var Vn={},En=function(){\n/*ThouShaltNotCache*/\nfunction e(e,t,r){\n/*ThouShaltNotCache*/\nvar i=this;void 0===t&&(t=Y),void 0===r&&(r=\"ObservableSet\"),this.name_=void 0,this[H]=Vn,this.data_=new Set,this.atom_=void 0,this.changeListeners_=void 0,this.interceptors_=void 0,this.dehancer=void 0,this.enhancer_=void 0,this.name_=r,p(Set)||n(22),this.enhancer_=function(e,n){\n/*ThouShaltNotCache*/\nreturn t(e,n,r)},ir(function(){\n/*ThouShaltNotCache*/\ni.atom_=F(i.name_),e&&i.replace(e)})}var t=e.prototype;return t.dehanceValue_=function(e){\n/*ThouShaltNotCache*/\nreturn void 0!==this.dehancer?this.dehancer(e):e},t.clear=function(){\n/*ThouShaltNotCache*/\nvar e=this;nn(function(){\n/*ThouShaltNotCache*/\not(function(){\n/*ThouShaltNotCache*/\nfor(var t,n=N(e.data_.values());!(t=n()).done;){var r=t.value;e.delete(r)}})})},t.forEach=function(e,t){\n/*ThouShaltNotCache*/\nfor(var n,r=N(this);!(n=r()).done;){var i=n.value;e.call(t,i,i,this)}},t.add=function(e){\n/*ThouShaltNotCache*/\nvar t=this;if(this.atom_,sn(this)){var n=un(this,{type:jn,object:this,newValue:e});if(!n)return this;e=n.newValue}if(!this.has(e)){nn(function(){\n/*ThouShaltNotCache*/\nt.data_.add(t.enhancer_(e,void 0)),t.atom_.reportChanged()});var r=!1,i=cn(this),o=i?{observableKind:\"set\",debugObjectName:this.name_,type:jn,object:this,newValue:e}:null;r,i&&hn(this,o)}return this},t.delete=function(e){\n/*ThouShaltNotCache*/\nvar t=this;if(sn(this)&&!un(this,{type:xn,object:this,oldValue:e}))return!1;if(this.has(e)){var n=cn(this),r=n?{observableKind:\"set\",debugObjectName:this.name_,type:xn,object:this,oldValue:e}:null;return nn(function(){\n/*ThouShaltNotCache*/\nt.atom_.reportChanged(),t.data_.delete(e)}),n&&hn(this,r),!0}return!1},t.has=function(e){\n/*ThouShaltNotCache*/\nreturn this.atom_.reportObserved(),this.data_.has(this.dehanceValue_(e))},t.entries=function(){\n/*ThouShaltNotCache*/\nvar e=this.values();return Cn({next:function(){\n/*ThouShaltNotCache*/\nvar t=e.next(),n=t.value,r=t.done;return r?{value:void 0,done:r}:{value:[n,n],done:r}}})},t.keys=function(){\n/*ThouShaltNotCache*/\nreturn this.values()},t.values=function(){\n/*ThouShaltNotCache*/\nthis.atom_.reportObserved();var e=this,t=this.data_.values();return Cn({next:function(){\n/*ThouShaltNotCache*/\nvar n=t.next(),r=n.value,i=n.done;return i?{value:void 0,done:i}:{value:e.dehanceValue_(r),done:i}}})},t.intersection=function(e){\n/*ThouShaltNotCache*/\nreturn k(e)&&!Rn(e)?e.intersection(this):new Set(this).intersection(e)},t.union=function(e){\n/*ThouShaltNotCache*/\nreturn k(e)&&!Rn(e)?e.union(this):new Set(this).union(e)},t.difference=function(e){\n/*ThouShaltNotCache*/\nreturn new Set(this).difference(e)},t.symmetricDifference=function(e){\n/*ThouShaltNotCache*/\nreturn k(e)&&!Rn(e)?e.symmetricDifference(this):new Set(this).symmetricDifference(e)},t.isSubsetOf=function(e){\n/*ThouShaltNotCache*/\nreturn new Set(this).isSubsetOf(e)},t.isSupersetOf=function(e){\n/*ThouShaltNotCache*/\nreturn new Set(this).isSupersetOf(e)},t.isDisjointFrom=function(e){\n/*ThouShaltNotCache*/\nreturn k(e)&&!Rn(e)?e.isDisjointFrom(this):new Set(this).isDisjointFrom(e)},t.replace=function(e){\n/*ThouShaltNotCache*/\nvar t=this;return Rn(e)&&(e=new Set(e)),nn(function(){\n/*ThouShaltNotCache*/\nArray.isArray(e)||k(e)?(t.clear(),e.forEach(function(e){\n/*ThouShaltNotCache*/\nreturn t.add(e)})):null!=e&&n(\"Cannot initialize set from \"+e)}),this},t.observe_=function(e,t){return ln(this,e)},t.intercept_=function(e){\n/*ThouShaltNotCache*/\nreturn an(this,e)},t.toJSON=function(){\n/*ThouShaltNotCache*/\nreturn Array.from(this)},t.toString=function(){\n/*ThouShaltNotCache*/\nreturn\"[object ObservableSet]\"},t[Symbol.iterator]=function(){\n/*ThouShaltNotCache*/\nreturn this.values()},D(e,[{key:\"size\",get:function(){\n/*ThouShaltNotCache*/\nreturn this.atom_.reportObserved(),this.data_.size}},{key:Symbol.toStringTag,get:function(){\n/*ThouShaltNotCache*/\nreturn\"Set\"}}])}(),Rn=S(\"ObservableSet\",En);function Cn(e){\n/*ThouShaltNotCache*/\nreturn e[Symbol.toStringTag]=\"SetIterator\",hr(e)}var Dn=Object.create(null),Nn=\"remove\",Ln=function(){\n/*ThouShaltNotCache*/\nfunction e(e,t,n,r){\n/*ThouShaltNotCache*/\nvoid 0===t&&(t=new Map),void 0===r&&(r=be),this.target_=void 0,this.values_=void 0,this.name_=void 0,this.defaultAnnotation_=void 0,this.keysAtom_=void 0,this.changeListeners_=void 0,this.interceptors_=void 0,this.proxy_=void 0,this.isPlainObject_=void 0,this.appliedAnnotations_=void 0,this.pendingKeys_=void 0,this.target_=e,this.values_=t,this.name_=n,this.defaultAnnotation_=r,this.keysAtom_=new W(\"ObservableObject.keys\"),this.isPlainObject_=y(this.target_)}var t=e.prototype;return t.getObservablePropValue_=function(e){\n/*ThouShaltNotCache*/\nreturn this.values_.get(e).get()},t.setObservablePropValue_=function(e,t){\n/*ThouShaltNotCache*/\nvar n=this.values_.get(e);if(n instanceof Xe)return n.set(t),!0;if(sn(this)){var r=un(this,{type:_n,object:this.proxy_||this.target_,name:e,newValue:t});if(!r)return null;t=r.newValue}if((t=n.prepareNewValue_(t))!==_t.UNCHANGED){var i=cn(this),o=i?{type:_n,observableKind:\"object\",debugObjectName:this.name_,object:this.proxy_||this.target_,oldValue:n.value_,name:e,newValue:t}:null;0,n.setNewValue_(t),i&&hn(this,o)}return!0},t.get_=function(e){\n/*ThouShaltNotCache*/\nreturn _t.trackingDerivation&&!P(this.target_,e)&&this.has_(e),this.target_[e]},t.set_=function(e,t,n){\n/*ThouShaltNotCache*/\nreturn void 0===n&&(n=!1),P(this.target_,e)?this.values_.has(e)?this.setObservablePropValue_(e,t):n?Reflect.set(this.target_,e,t):(this.target_[e]=t,!0):this.extend_(e,{value:t,enumerable:!0,writable:!0,configurable:!0},this.defaultAnnotation_,n)},t.has_=function(e){\n/*ThouShaltNotCache*/\nif(!_t.trackingDerivation)return e in this.target_;this.pendingKeys_||(this.pendingKeys_=new Map);var t=this.pendingKeys_.get(e);return t||(t=new Fe(e in this.target_,J,\"ObservableObject.key?\",!1),this.pendingKeys_.set(e,t)),t.get()},t.make_=function(e,t){if(\n/*ThouShaltNotCache*/\n!0===t&&(t=this.defaultAnnotation_),!1!==t){if(qn(this,t,e),!(e in this.target_)){var r;if(null!=(r=this.target_[K])&&r[e])return;n(1,t.annotationType_,this.name_+\".\"+e.toString())}for(var i=this.target_;i&&i!==u;){var o=s(i,e);if(o){var a=t.make_(this,e,o,i);if(0===a)return;if(1===a)break}i=Object.getPrototypeOf(i)}Gn(this,t,e)}},t.extend_=function(e,t,n,r){if(\n/*ThouShaltNotCache*/\nvoid 0===r&&(r=!1),!0===n&&(n=this.defaultAnnotation_),!1===n)return this.defineProperty_(e,t,r);qn(this,n,e);var i=n.extend_(this,e,t,r);return i&&Gn(this,n,e),i},t.defineProperty_=function(e,t,n){\n/*ThouShaltNotCache*/\nvoid 0===n&&(n=!1),this.keysAtom_;try{bt();var r=this.delete_(e);if(!r)return r;if(sn(this)){var i=un(this,{object:this.proxy_||this.target_,name:e,type:jn,newValue:t.value});if(!i)return null;var o=i.newValue;t.value!==o&&(t=L({},t,{value:o}))}if(n){if(!Reflect.defineProperty(this.target_,e,t))return!1}else a(this.target_,e,t);this.notifyPropertyAddition_(e,t.value)}finally{yt()}return!0},t.defineObservableProperty_=function(e,t,n,r){\n/*ThouShaltNotCache*/\nvoid 0===r&&(r=!1),this.keysAtom_;try{bt();var i=this.delete_(e);if(!i)return i;if(sn(this)){var o=un(this,{object:this.proxy_||this.target_,name:e,type:jn,newValue:t});if(!o)return null;t=o.newValue}var s=In(e),u={configurable:!_t.safeDescriptors||this.isPlainObject_,enumerable:!0,get:s.get,set:s.set};if(r){if(!Reflect.defineProperty(this.target_,e,u))return!1}else a(this.target_,e,u);var c=new Fe(t,n,\"ObservableObject.key\",!1);this.values_.set(e,c),this.notifyPropertyAddition_(e,c.value_)}finally{yt()}return!0},t.defineComputedProperty_=function(e,t,n){\n/*ThouShaltNotCache*/\nvoid 0===n&&(n=!1),this.keysAtom_;try{bt();var r=this.delete_(e);if(!r)return r;if(sn(this))if(!un(this,{object:this.proxy_||this.target_,name:e,type:jn,newValue:void 0}))return null;t.name||(t.name=\"ObservableObject.key\"),t.context=this.proxy_||this.target_;var i=In(e),o={configurable:!_t.safeDescriptors||this.isPlainObject_,enumerable:!1,get:i.get,set:i.set};if(n){if(!Reflect.defineProperty(this.target_,e,o))return!1}else a(this.target_,e,o);this.values_.set(e,new Xe(t)),this.notifyPropertyAddition_(e,void 0)}finally{yt()}return!0},t.delete_=function(e,t){if(\n/*ThouShaltNotCache*/\nvoid 0===t&&(t=!1),this.keysAtom_,!P(this.target_,e))return!0;if(sn(this)&&!un(this,{object:this.proxy_||this.target_,name:e,type:Nn}))return null;try{var n;bt();var r,i=cn(this),o=this.values_.get(e),a=void 0;if(!o&&i)a=null==(r=s(this.target_,e))?void 0:r.value;if(t){if(!Reflect.deleteProperty(this.target_,e))return!1}else delete this.target_[e];if(o&&(this.values_.delete(e),o instanceof Fe&&(a=o.value_),Ot(o)),this.keysAtom_.reportChanged(),null==(n=this.pendingKeys_)||null==(n=n.get(e))||n.set(e in this.target_),i){var u={type:Nn,observableKind:\"object\",object:this.proxy_||this.target_,debugObjectName:this.name_,oldValue:a,name:e};0,i&&hn(this,u)}}finally{yt()}return!0},t.observe_=function(e,t){return ln(this,e)},t.intercept_=function(e){\n/*ThouShaltNotCache*/\nreturn an(this,e)},t.notifyPropertyAddition_=function(e,t){\n/*ThouShaltNotCache*/\nvar n,r=cn(this);if(r){var i=r?{type:jn,observableKind:\"object\",debugObjectName:this.name_,object:this.proxy_||this.target_,name:e,newValue:t}:null;0,r&&hn(this,i)}null==(n=this.pendingKeys_)||null==(n=n.get(e))||n.set(!0),this.keysAtom_.reportChanged()},t.ownKeys_=function(){\n/*ThouShaltNotCache*/\nreturn this.keysAtom_.reportObserved(),x(this.target_)},t.keys_=function(){\n/*ThouShaltNotCache*/\nreturn this.keysAtom_.reportObserved(),Object.keys(this.target_)},e}();function Bn(e,t){\n/*ThouShaltNotCache*/\nvar n;if(P(e,H))return e;var r=null!=(n=null==t?void 0:t.name)?n:\"ObservableObject\",i=new Ln(e,new Map,String(r),function(e){\n/*ThouShaltNotCache*/\nvar t;return e?null!=(t=e.defaultDecorator)?t:ye(e):void 0}(t));return O(e,H,i),e}var Un=S(\"ObservableObjectAdministration\",Ln);function In(e){\n/*ThouShaltNotCache*/\nreturn Dn[e]||(Dn[e]={get:function(){\n/*ThouShaltNotCache*/\nreturn this[H].getObservablePropValue_(e)},set:function(t){\n/*ThouShaltNotCache*/\nreturn this[H].setObservablePropValue_(e,t)}})}function Kn(e){\n/*ThouShaltNotCache*/\nreturn!!b(e)&&Un(e[H])}function Gn(e,t,n){\n/*ThouShaltNotCache*/\nvar r;null==(r=e.target_[K])||delete r[n]}function qn(e,t,n){}var zn,Hn,Wn=Jn(0),$n=function(){\n/*ThouShaltNotCache*/\nvar e=!1,t={};return Object.defineProperty(t,\"0\",{set:function(){\n/*ThouShaltNotCache*/\ne=!0}}),Object.create(t)[0]=1,!1===e}(),Fn=0,Xn=function(){};zn=Xn,Hn=Array.prototype,\n/*ThouShaltNotCache*/\nObject.setPrototypeOf?Object.setPrototypeOf(zn.prototype,Hn):void 0!==zn.prototype.__proto__?zn.prototype.__proto__=Hn:zn.prototype=Hn;var Yn=function(e){\n/*ThouShaltNotCache*/\nfunction t(t,n,r,i){\n/*ThouShaltNotCache*/\nvar o;return void 0===r&&(r=\"ObservableArray\"),void 0===i&&(i=!1),o=e.call(this)||this,ir(function(){\n/*ThouShaltNotCache*/\nvar e=new pn(r,n,i,!0);e.proxy_=o,w(o,H,e),t&&t.length&&o.spliceWithArray(0,0,t),$n&&Object.defineProperty(o,\"0\",Wn)}),o}B(t,e);var n=t.prototype;return n.concat=function(){\n/*ThouShaltNotCache*/\nthis[H].atom_.reportObserved();for(var e=arguments.length,t=new Array(e),n=0;n<e;n++)t[n]=arguments[n];return Array.prototype.concat.apply(this.slice(),t.map(function(e){\n/*ThouShaltNotCache*/\nreturn An(e)?e.slice():e}))},n[Symbol.iterator]=function(){\n/*ThouShaltNotCache*/\nvar e=this,t=0;return hr({next:function(){\n/*ThouShaltNotCache*/\nreturn t<e.length?{value:e[t++],done:!1}:{done:!0,value:void 0}}})},D(t,[{key:\"length\",get:function(){\n/*ThouShaltNotCache*/\nreturn this[H].getArrayLength_()},set:function(e){\n/*ThouShaltNotCache*/\nthis[H].setArrayLength_(e)}},{key:Symbol.toStringTag,get:function(){\n/*ThouShaltNotCache*/\nreturn\"Array\"}}])}(Xn);function Jn(e){\n/*ThouShaltNotCache*/\nreturn{enumerable:!1,configurable:!0,get:function(){\n/*ThouShaltNotCache*/\nreturn this[H].get_(e)},set:function(t){\n/*ThouShaltNotCache*/\nthis[H].set_(e,t)}}}function Qn(e){\n/*ThouShaltNotCache*/\na(Yn.prototype,\"\"+e,Jn(e))}function Zn(e){\n/*ThouShaltNotCache*/\nif(e>Fn){for(var t=Fn;t<e+100;t++)Qn(t);Fn=e}}function er(e,t,n){\n/*ThouShaltNotCache*/\nreturn new Yn(e,t,n)}function tr(e,t){\n/*ThouShaltNotCache*/\nif(\"object\"==typeof e&&null!==e){if(An(e))return void 0!==t&&n(23),e[H].atom_;if(Rn(e))return e.atom_;if(Pn(e)){if(void 0===t)return e.keysAtom_;var r=e.data_.get(t)||e.hasMap_.get(t);return r||n(25,t,rr(e)),r}if(Kn(e)){if(!t)return n(26);var i=e[H].values_.get(t);return i||n(27,t,rr(e)),i}if($(e)||Qe(e)||xt(e))return e}else if(p(e)&&xt(e[H]))return e[H];n(28)}function nr(e,t){\n/*ThouShaltNotCache*/\nreturn e||n(29),void 0!==t?nr(tr(e,t)):$(e)||Qe(e)||xt(e)||Pn(e)||Rn(e)?e:e[H]?e[H]:void n(24,e)}function rr(e,t){\n/*ThouShaltNotCache*/\nvar n;if(void 0!==t)n=tr(e,t);else{if(Bt(e))return e.name;n=Kn(e)||Pn(e)||Rn(e)?nr(e):tr(e)}return n.name_}function ir(e){\n/*ThouShaltNotCache*/\nvar t=st(),n=We(!0);bt();try{return e()}finally{yt(),$e(n),at(t)}}Object.entries(bn).forEach(function(e){\n/*ThouShaltNotCache*/\nvar t=e[0],n=e[1];\"concat\"!==t&&O(Yn.prototype,t,n)}),Zn(1e3);var or,sr=u.toString;function ar(e,t,n){\n/*ThouShaltNotCache*/\nreturn void 0===n&&(n=-1),ur(e,t,n)}function ur(e,t,n,r,i){\n/*ThouShaltNotCache*/\nif(e===t)return 0!==e||1/e==1/t;if(null==e||null==t)return!1;if(e!=e)return t!=t;var o=typeof e;if(\"function\"!==o&&\"object\"!==o&&\"object\"!=typeof t)return!1;var s=sr.call(e);if(s!==sr.call(t))return!1;switch(s){case\"[object RegExp]\":case\"[object String]\":return\"\"+e==\"\"+t;case\"[object Number]\":return+e!=+e?+t!=+t:0===+e?1/+e==1/t:+e===+t;case\"[object Date]\":case\"[object Boolean]\":return+e===+t;case\"[object Symbol]\":return\"undefined\"!=typeof Symbol&&Symbol.valueOf.call(e)===Symbol.valueOf.call(t);case\"[object Map]\":case\"[object Set]\":n>=0&&n++}e=cr(e),t=cr(t);var a=\"[object Array]\"===s;if(!a){if(\"object\"!=typeof e||\"object\"!=typeof t)return!1;var u=e.constructor,c=t.constructor;if(u!==c&&!(p(u)&&u instanceof u&&p(c)&&c instanceof c)&&\"constructor\"in e&&\"constructor\"in t)return!1}if(0===n)return!1;n<0&&(n=-1),i=i||[];for(var l=(r=r||[]).length;l--;)if(r[l]===e)return i[l]===t;if(r.push(e),i.push(t),a){if((l=e.length)!==t.length)return!1;for(;l--;)if(!ur(e[l],t[l],n-1,r,i))return!1}else{var h=Object.keys(e),f=h.length;if(Object.keys(t).length!==f)return!1;for(var d=0;d<f;d++){var _=h[d];if(!P(t,_)||!ur(e[_],t[_],n-1,r,i))return!1}}return r.pop(),i.pop(),!0}function cr(e){\n/*ThouShaltNotCache*/\nreturn An(e)?e.slice():A(e)||Pn(e)||k(e)||Rn(e)?Array.from(e.entries()):e}var lr=(null==(or=i().Iterator)?void 0:or.prototype)||{};function hr(e){\n/*ThouShaltNotCache*/\nreturn e[Symbol.iterator]=fr,Object.assign(Object.create(lr),e)}function fr(){\n/*ThouShaltNotCache*/\nreturn this}[\"Symbol\",\"Map\",\"Set\"].forEach(function(e){void 0===i()[e]&&n(\"MobX requires global '\"+e+\"' to be available or polyfilled\")}),\"object\"==typeof __MOBX_DEVTOOLS_GLOBAL_HOOK__&&__MOBX_DEVTOOLS_GLOBAL_HOOK__.injectMobx({spy:function(e){return console.warn(\"[mobx.spy] Is a no-op in production builds\"),function(){}},extras:{getDebugName:rr},$mobx:H});class dr{id;name;lastMessage=null;rooms=new Set;status=\"Online\";constructor(e,t){\n/*ThouShaltNotCache*/\nfn(this,{name:Ce,lastMessage:Ce,rooms:Ce,status:Ce,setName:Nt,setLastMessage:Nt,addRoom:Nt,removeRoom:Nt,setStatus:Nt}),this.id=e,this.name=t}setName(e){\n/*ThouShaltNotCache*/\nthis.name=e}setLastMessage(e){\n/*ThouShaltNotCache*/\nthis.lastMessage=e}addRoom(e){\n/*ThouShaltNotCache*/\nthis.rooms.add(e)}removeRoom(e){\n/*ThouShaltNotCache*/\nthis.rooms.delete(e)}setStatus(e){\n/*ThouShaltNotCache*/\nthis.status=e,this.rooms.forEach(t=>{\n/*ThouShaltNotCache*/\nt.addStatusUpdate(this,e)})}}class _r{name;people=new Map;messages=[];roomUpdates=[];constructor(e){\n/*ThouShaltNotCache*/\nfn(this,{name:Ce,people:Ce,messages:Ce,roomUpdates:Ce,messageCount:Be,peopleCount:Be,lastMessage:Be,members:Be,addPerson:Nt,removePerson:Nt,addMessage:Nt,addStatusUpdate:Nt,dispose:Nt}),this.name=e}get messageCount(){\n/*ThouShaltNotCache*/\nreturn this.messages.length}get peopleCount(){\n/*ThouShaltNotCache*/\nreturn this.people.size}get lastMessage(){\n/*ThouShaltNotCache*/\nreturn 0===this.messages.length?null:this.messages[this.messages.length-1]}get members(){\n/*ThouShaltNotCache*/\nreturn Array.from(this.people.values())}addPerson(e){\n/*ThouShaltNotCache*/\nthis.people.has(e.id)||(this.people.set(e.id,e),e.addRoom(this))}removePerson(e){\n/*ThouShaltNotCache*/\nconst t=this.people.get(e);t&&(t.removeRoom(this),this.people.delete(e))}addMessage(e){\n/*ThouShaltNotCache*/\nthis.messages.push(e),e.author.setLastMessage(e)}addStatusUpdate(e,t){\n/*ThouShaltNotCache*/\nthis.roomUpdates.push(`${e.name} is now ${t}`),this.roomUpdates.length>20&&(this.roomUpdates=this.roomUpdates.slice(10))}dispose(e){\n/*ThouShaltNotCache*/\nthis.people.forEach(e=>e.removeRoom(this)),e&&e.removeRoom(this.name)}}class vr{static _nextId=0;id;text;author;timestamp;static nextId(){\n/*ThouShaltNotCache*/\nreturn vr._nextId++}constructor(e,t){\n/*ThouShaltNotCache*/\nfn(this,{text:Ce,updateText:Nt}),this.id=vr.nextId(),this.text=e,this.author=t,this.timestamp=new Date}updateText(e){\n/*ThouShaltNotCache*/\nthis.text=e}}class pr{lastMessages=new Map;lastMessage;roomDisposers=new Map;constructor(e){\n/*ThouShaltNotCache*/\nfn(this,{lastMessages:Ce,updateLastMessage:Nt,addRoom:Nt,removeRoom:Nt}),e.forEach(e=>this.addRoom(e))}addRoom(e){\n/*ThouShaltNotCache*/\nconst t=function(e,t,n){\n/*ThouShaltNotCache*/\nvar r,i,o;void 0===n&&(n=l);var s,a,u,c=null!=(r=n.name)?r:\"Reaction\",h=Nt(c,n.onError?(s=n.onError,a=t,function(){\n/*ThouShaltNotCache*/\ntry{return a.apply(this,arguments)}catch(e){s.call(this,e)}}):t),f=!n.scheduler&&!n.delay,d=Kt(n),_=!0,v=!1,p=n.compareStructural?X.structural:n.equals||X.default,g=new wt(c,function(){\n/*ThouShaltNotCache*/\n_||f?b():v||(v=!0,d(b))},n.onError,n.requiresObservable);function b(){if(\n/*ThouShaltNotCache*/\nv=!1,!g.isDisposed){var t=!1,r=u;g.track(function(){\n/*ThouShaltNotCache*/\nvar n=He(!1,function(){\n/*ThouShaltNotCache*/\nreturn e(g)});t=_||!p(u,n),u=n}),(_&&n.fireImmediately||!_&&t)&&h(u,r,g),_=!1}}return null!=(i=n)&&null!=(i=i.signal)&&i.aborted||g.schedule_(),g.getDisposer_(null==(o=n)?void 0:o.signal)}(()=>e.lastMessage,t=>{\n/*ThouShaltNotCache*/\nt&&this.updateLastMessage(e.name,t)});this.roomDisposers.set(e.name,t)}removeRoom(e){\n/*ThouShaltNotCache*/\nthis.roomDisposers.has(e)&&(this.roomDisposers.get(e)(),this.roomDisposers.delete(e),this.lastMessages.delete(e))}updateLastMessage(e,t){\n/*ThouShaltNotCache*/\nthis.lastMessages.set(e,t)}}function gr(e){\n/*ThouShaltNotCache*/\nif(!e)throw new Error(\"Assertion failure\")}function br(){\n/*ThouShaltNotCache*/\nconst e=[],t=(...t)=>e.push(t.join(\" \")),n=new _r(\"Tech Talk\"),r=new _r(\"General Chat\"),i=new _r(\"Large Meeting Room\"),o=new dr(1,\"Alice\"),s=new dr(2,\"Bob\");n.addPerson(o),n.addPerson(s),r.addPerson(o);const a=[n,r,i],u=new pr(a);t(\"\\n--- Setting up Large Meeting Room ---\");const c=[];for(let e=3;e<203;e++){const t=new dr(e,`Person ${e}`);c.push(t),i.addPerson(t)}t(`[INFO] Added ${i.peopleCount} members to the large room.`),i.addPerson(o),i.addPerson(s),t(\"[INFO] Added Alice and Bob to the Large Meeting Room.\"),gr(3===o.rooms.size),gr(o.rooms.has(n)&&o.rooms.has(r)&&o.rooms.has(i)),gr(2===s.rooms.size&&s.rooms.has(n)&&s.rooms.has(i)),t(\"[PASS] Verified Person.rooms associations with Set.\");const l=Ut(()=>{\n/*ThouShaltNotCache*/\nt(\"\\n--- Global Notifications (Last Messages) ---\"),0===u.lastMessages.size?t(\"No messages yet.\"):u.lastMessages.forEach((e,n)=>{\n/*ThouShaltNotCache*/\nt(`[${n}] \"${e.text}\" - ${e.author.name}`)}),t(\"------------------------------------------\")}),h=Ut(()=>{\n/*ThouShaltNotCache*/\na.forEach(e=>{\n/*ThouShaltNotCache*/\ne.roomUpdates.length>0&&t(`[${e.name} Update] ${e.roomUpdates[e.roomUpdates.length-1]}`)})});t(\"\\n--- Status Update Scenario ---\"),o.setStatus(\"Away\"),gr(n.roomUpdates.includes(\"Alice is now Away\")),gr(r.roomUpdates.includes(\"Alice is now Away\")),gr(i.roomUpdates.includes(\"Alice is now Away\")),t(\"[PASS] Verified that status updates are sent to all of Alice's rooms.\"),s.setStatus(\"Busy\"),gr(n.roomUpdates.includes(\"Bob is now Busy\")),gr(!r.roomUpdates.includes(\"Bob is now Busy\")),gr(i.roomUpdates.includes(\"Bob is now Busy\")),t(\"[PASS] Verified that status updates are only sent to Bob's rooms.\"),t(\"\\n--- Sending Messages & Verifying Notifications ---\");const f=new vr(\"First message to Tech Talk!\",o);n.addMessage(f),gr(u.lastMessages.get(\"Tech Talk\")===f),t(\"[PASS] Verified Tech Talk notification.\");for(let e=0;e<20;e++){const t=new vr(`ping ${e}`,o);r.addMessage(t),gr(u.lastMessages.get(\"General Chat\")===t)}t(\"[PASS] Verified General Chat notification.\");for(let e=0;e<=5;e++)for(const e of c){const t=new vr(`Hi, I am ${e.name}`,e);i.addMessage(t),gr(i.lastMessage==t),gr(u.lastMessages.get(\"Large Meeting Room\")===t)}t(\"[PASS] Verified Large Meeting Room notification.\");for(let e=0;e<20;e++){const t=new vr(`ping ${e}`,s);n.addMessage(t),gr(u.lastMessages.get(\"Tech Talk\")===t)}t(\"[PASS] Verified Tech Talk notification update.\");const d=[];for(let e=0;e<10;e++){const t=`General Chat ${e}`,n=new _r(t);d.push(n),u.addRoom(n);for(const e of c.slice(0,10))n.addPerson(e);for(const r of n.members.slice(0,5)){const i=new vr(`ping ${e}`,r);n.addMessage(i),gr(u.lastMessages.get(t)===i)}}return d.forEach(e=>e.dispose(u)),l(),h(),n.dispose(u),r.dispose(u),i.dispose(u),e}})(),MobXBenchmark=t})();\n//# sourceMappingURL=bundle.es6.min.js.map","./utils/StartupBenchmark.js":"/*\n * Copyright (C) 2025 Apple Inc. All rights reserved.\n * Copyright 2025 Google LLC\n * \n * Redistribution and use in source and binary forms, with or without\n * modification, are permitted provided that the following conditions\n * are met:\n * 1. Redistributions of source code must retain the above copyright\n *    notice, this list of conditions and the following disclaimer.\n * 2. Redistributions in binary form must reproduce the above copyright\n *    notice, this list of conditions and the following disclaimer in the\n *    documentation and/or other materials provided with the distribution.\n *\n * THIS SOFTWARE IS PROVIDED BY APPLE INC. AND ITS CONTRIBUTORS ``AS IS''\n * AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO,\n * THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR\n * PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL APPLE INC. OR ITS CONTRIBUTORS\n * BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR\n * CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF\n * SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS\n * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN\n * CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)\n * ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF\n * THE POSSIBILITY OF SUCH DAMAGE.\n */\n\nconst CACHE_BUST_COMMENT = \"/*ThouShaltNotCache*/\";\nconst CACHE_BUST_COMMENT_RE = new RegExp(\n  `\\n${RegExp.escape(CACHE_BUST_COMMENT)}\\n`,\n  \"g\"\n);\n\nclass StartupBenchmark {\n  // Total iterations for this benchmark.\n  #iterationCount = 0;\n  // Original source code.\n  #sourceCode;\n  // quickHahs(this.#sourceCode) for use in custom validate() methods.\n  #sourceHash = 0;\n  // Number of no-cache comments in the original #sourceCode.\n  #expectedCacheCommentCount = 0;\n  // How many times (separate iterations) should we reuse the source code.\n  // Use 0 to skip and only use a single #sourceCode string.\n  #sourceCodeReuseCount = 1;\n  // #sourceCode for each iteration, number of unique sources is controlled\n  // by codeReuseCount;\n  #iterationSourceCodes = [];\n\n  constructor({\n    iterationCount,\n    expectedCacheCommentCount,\n    sourceCodeReuseCount = 1,\n  } = {}) {\n    console.assert(\n      iterationCount > 0,\n      `Expected iterationCount to be positive, but got ${iterationCount}`\n    );\n    this.#iterationCount = iterationCount;\n    console.assert(\n      expectedCacheCommentCount > 0,\n      `Expected expectedCacheCommentCount to be positive, but got ${expectedCacheCommentCount}`\n    );\n    this.#expectedCacheCommentCount = expectedCacheCommentCount;\n    console.assert(\n      sourceCodeReuseCount >= 0,\n      `Expected sourceCodeReuseCount to be non-negative, but got ${sourceCodeReuseCount}`\n    );\n    this.#sourceCodeReuseCount = sourceCodeReuseCount;\n  }\n\n  get iterationCount() {\n    return this.#iterationCount;\n  }\n\n  get sourceCode() {\n    return this.#sourceCode;\n  }\n\n  get sourceHash() {\n    return this.#sourceHash;\n  }\n\n  get expectedCacheCommentCount() {\n    return this.#expectedCacheCommentCount;\n  }\n\n  get sourceCodeReuseCount() {\n    return this.#sourceCodeReuseCount;\n  }\n\n  get iterationSourceCodes() {\n    return this.#iterationSourceCodes;\n  }\n\n  async init() {\n    if (!JetStream.preload.BUNDLE) {\n      throw new Error(\"Missing JetStream.preload.BUNDLE\");\n    }\n    this.#sourceCode = await JetStream.getString(JetStream.preload.BUNDLE);\n    if (!this.sourceCode || !this.sourceCode.length) {\n      throw new Error(\"Couldn't load JetStream.preload.BUNDLE\");\n    }\n\n    const cacheCommentCount = this.sourceCode.match(\n      CACHE_BUST_COMMENT_RE\n    ).length;\n    this.#sourceHash = this.quickHash(this.sourceCode);\n    this.validateSourceCacheComments(cacheCommentCount);\n    for (let i = 0; i < this.iterationCount; i++)\n      this.#iterationSourceCodes[i] = this.createIterationSourceCode(i);\n    this.validateIterationSourceCodes();\n  }\n\n  validateSourceCacheComments(cacheCommentCount) {\n    console.assert(\n      cacheCommentCount === this.expectedCacheCommentCount,\n      `Invalid cache comment count ${cacheCommentCount} expected ${this.expectedCacheCommentCount}.`\n    );\n  }\n\n  validateIterationSourceCodes() {\n    if (this.#iterationSourceCodes.some((each) => !each?.length))\n      throw new Error(`Got invalid iterationSourceCodes`);\n    let expectedSize = 1;\n    if (this.sourceCodeReuseCount !== 0)\n      expectedSize = Math.ceil(this.iterationCount / this.sourceCodeReuseCount);\n    const uniqueSources = new Set(this.iterationSourceCodes);\n    if (uniqueSources.size != expectedSize)\n      throw new Error(\n        `Expected ${expectedSize} unique sources, but got ${uniqueSources.size}.`\n      );\n  }\n\n  createIterationSourceCode(iteration) {\n    // Alter the code per iteration to prevent caching.\n    const cacheId =\n      Math.floor(iteration / this.sourceCodeReuseCount) *\n      this.sourceCodeReuseCount;\n    // Reuse existing sources if this.codeReuseCount > 1:\n    if (cacheId < this.iterationSourceCodes.length)\n      return this.iterationSourceCodes[cacheId];\n\n    const sourceCode = this.sourceCode.replaceAll(\n      CACHE_BUST_COMMENT_RE,\n      `/*${cacheId}*/`\n    );\n    // Warm up quickHash.\n    this.quickHash(sourceCode);\n    return sourceCode;\n  }\n\n  quickHash(str) {\n    let hash = 5381;\n    let i = str.length;\n    while (i > 0) {\n      hash = (hash * 33) ^ (str.charCodeAt(i) | 0);\n      i -= 919;\n    }\n    return hash | 0;\n  }\n}\n"};
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
