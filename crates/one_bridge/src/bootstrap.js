// Sentinel Plugin Bootstrap for One Engine
// Provides the Sentinel.* JS API surface that plugins use.

// Initialize sentinel storage arrays (these are read by the Rust runtime).
var __sentinel_findings__ = [];
var __sentinel_logs__ = [];
var __sentinel_last_result__ = undefined;

var Sentinel = {
    log: function(level, message) {
        __sentinel_log(level, message);
    },
    emitFinding: function(finding) {
        __sentinel_emit_finding(finding);
    },
    "return": function(value) {
        __sentinel_return(value);
    },

    TLS: {
        getCertificate: function(host, port) {
            return __tls_get_certificate(host, port || 443);
        },
        peerCertificate: function() {
            return __tls_peer_certificate();
        }
    },

    Network: {
        scanPorts: function(host, ports) {
            return __network_scan_ports(host, ports);
        },
        probeServices: function(targets) {
            return __network_probe_services(targets);
        },
        getServiceProbeCapabilities: function() {
            return __network_get_capabilities();
        }
    },

    Monitor: {
        reportProgress: function(data) {
            return __monitor_report_progress(data);
        },
        emitActiveProbeEvent: function(event) {
            return __monitor_emit_active_probe_event(event);
        },
        getSettings: function() {
            return __monitor_get_settings();
        }
    },

    Dictionary: {
        get: function(id) {
            return __dict_get_dictionary(id);
        },
        getDefaultId: function() {
            return __dict_get_default_id();
        },
        getWords: function(id) {
            return __dict_get_words(id);
        },
        getEntries: function(id) {
            return __dict_get_entries(id);
        },
        list: function() {
            return __dict_list();
        }
    },

    AST: {
        parse: function(source) {
            return __ast_parse_js(source);
        }
    }
};

// Deno compatibility stubs
var Deno = {
    readTextFile: function(path) {
        return __fs_read_text_file(path);
    },
    writeTextFile: function(path, content) {
        return __fs_write_text_file(path, content);
    },
    readFile: function(path) {
        return __fs_read_file(path);
    },
    writeFile: function(path, data) {
        return __fs_write_file(path, data);
    },
    mkdir: function(path, options) {
        return __fs_mkdir(path, options && options.recursive);
    },
    readDir: function(path) {
        return __fs_read_dir(path);
    },
    stat: function(path) {
        return __fs_stat(path);
    },
    copyFile: function(src, dst) {
        return __fs_copy_file(src, dst);
    },
    remove: function(path, options) {
        return __fs_remove(path, options && options.recursive);
    },
    makeTempFile: function() {
        return __fs_make_temp_file();
    },
    build: {
        os: "unknown",
        arch: "unknown"
    }
};

// fetch polyfill
function fetch(url, options) {
    return __sentinel_fetch(url, options);
}
