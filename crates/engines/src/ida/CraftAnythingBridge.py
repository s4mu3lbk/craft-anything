"""Craft Anything's headless IDA bridge."""

import hmac
import json
import os
import socket
import traceback

import ida_auto
import ida_funcs
import ida_hexrays
import ida_kernwin
import ida_nalt
import ida_segment
import idautils
import idc

MAX_REQUEST_BYTES = 1024 * 1024
MAX_CODE_CHARACTERS = 8 * 1024 * 1024
MAX_FUNCTIONS = 200000


def write_json(path, value):
    with open(path, "w", encoding="utf-8") as out:
        json.dump(value, out, separators=(",", ":"))


def send(client, request_id, result=None, error=None):
    message = {"id": request_id, "ok": error is None}
    if error is None:
        message["result"] = result
    else:
        message["error"] = error
    client.sendall((json.dumps(message, separators=(",", ":")) + "\n").encode("utf-8"))


def function_name(function):
    return ida_funcs.get_func_name(function.start_ea) or "sub_%X" % function.start_ea


def resolve_function(address, made):
    function = ida_funcs.get_func(address)
    if function is not None:
        return function, False
    segment = ida_segment.getseg(address)
    if segment is None or not (segment.perm & ida_segment.SEGPERM_EXEC):
        raise ValueError("the address is not in executable memory")
    ida_auto.auto_make_code(address)
    if not ida_funcs.add_func(address):
        raise ValueError("IDA could not make a function at this address")
    function = ida_funcs.get_func(address)
    if function is None:
        raise ValueError("IDA could not make a function at this address")
    made.add(function.start_ea)
    return function, True


def dispatch(method, params, decompiler_available, made):
    if method == "ping":
        return {
            "name": "CraftAnything IDA bridge",
            "provider": {"id": "ida", "version": ida_kernwin.get_kernel_version()},
            "analysed": True,
            "analysis_cut_short": False,
            "decompiler_available": decompiler_available,
            "target": {"image_base": "0x%X" % ida_nalt.get_imagebase()},
        }
    if method == "native_functions_list":
        functions = []
        truncated = False
        for index, address in enumerate(idautils.Functions()):
            if index >= MAX_FUNCTIONS:
                truncated = True
                break
            functions.append({"name": ida_funcs.get_func_name(address) or "sub_%X" % address, "address": "0x%X" % address})
        return {"functions": functions, "truncated": truncated}
    if method == "native_function_pseudocode":
        if not decompiler_available:
            raise RuntimeError("this IDA installation has no usable decompiler")
        address = int(params.get("address", ""), 16)
        if address < 0 or address > 0xFFFFFFFFFFFFFFFF:
            raise ValueError("the address is outside IDA's supported range")
        function, created = resolve_function(address, made)
        try:
            decompiled = ida_hexrays.decompile(function.start_ea)
        except Exception as error:
            raise RuntimeError("IDA could not decompile this function: %s" % str(error)[:2000])
        if decompiled is None:
            raise RuntimeError("IDA returned no decompilation for this function")
        pseudocode = str(decompiled)
        if len(pseudocode) > MAX_CODE_CHARACTERS:
            raise RuntimeError("the pseudocode exceeds the bridge's size limit")
        signature = idc.get_type(function.start_ea)
        return {
            "name": function_name(function),
            "address": "0x%X" % function.start_ea,
            "signature": signature,
            "pseudocode": pseudocode,
            "created": created or function.start_ea in made,
        }
    raise LookupError("unknown method")


def serve(descriptor, decompiler_available):
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(("127.0.0.1", 0))
    server.listen(1)
    server.settimeout(300)
    write_json(descriptor["endpoint_path"], {"host": "127.0.0.1", "port": server.getsockname()[1]})
    made = set()
    try:
        client, _ = server.accept()
        client.settimeout(300)
        with client:
            reader = client.makefile("rb")
            while True:
                line = reader.readline(MAX_REQUEST_BYTES + 1)
                if not line:
                    break
                if len(line) > MAX_REQUEST_BYTES:
                    break
                if not line.strip():
                    continue
                request_id = 0
                try:
                    request = json.loads(line)
                    request_id = int(request.get("id", 0))
                    token = str(request.get("token", ""))
                    if not hmac.compare_digest(token, descriptor["token"]):
                        send(client, request_id, error={"code": "bad_input", "message": "bad token"})
                        break
                    method = str(request.get("method", ""))
                    if method == "shutdown":
                        send(client, request_id, result={"shutdown": True})
                        break
                    params = request.get("params") or {}
                    result = dispatch(method, params, decompiler_available, made)
                    send(client, request_id, result=result)
                except LookupError as error:
                    send(client, request_id, error={"code": "unknown_method", "message": str(error)[:4000]})
                except ValueError as error:
                    send(client, request_id, error={"code": "not_found", "message": str(error)[:4000]})
                except Exception as error:
                    send(client, request_id, error={"code": "bridge_error", "message": str(error)[:4000]})
    finally:
        server.close()


def main():
    descriptor_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "session.json")
    with open(descriptor_path, "r", encoding="utf-8") as source:
        descriptor = json.load(source)
    if not ida_auto.auto_wait():
        raise RuntimeError("IDA auto-analysis did not finish")
    if descriptor.get("prepare_only"):
        if not idc.save_database(descriptor["database_path"]):
            raise RuntimeError("IDA could not save its analysis database")
        idc.qexit(0)
        return
    decompiler_available = bool(ida_hexrays.init_hexrays_plugin())
    serve(descriptor, decompiler_available)
    idc.qexit(0)


try:
    main()
except Exception:
    try:
        error_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "bridge-error.txt")
        with open(error_path, "w", encoding="utf-8") as out:
            out.write(traceback.format_exc()[:16000])
    finally:
        idc.qexit(1)
