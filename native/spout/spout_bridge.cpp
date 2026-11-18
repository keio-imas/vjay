// GPU-only Direct3D 12 / Spout bridge. Spout2 is built from its pinned BSD source.
#include <SpoutDX12.h>
#include <algorithm>
#include <cstring>
#include <new>
struct Endpoint : spoutDX12 {
    ID3D11Resource* wrapped = nullptr;
    ID3D12Resource* resource = nullptr;
    ~Endpoint() { if (wrapped) wrapped->Release(); CloseDirectX12(); }
    bool bind(ID3D12Resource* texture, D3D12_RESOURCE_STATES state) {
        if (resource == texture && wrapped) return true;
        if (wrapped) wrapped->Release();
        wrapped = nullptr; resource = texture;
        D3D11_RESOURCE_FLAGS flags = {};
        return SUCCEEDED(GetD3D11On12device()->CreateWrappedResource(texture, &flags, state, state, IID_PPV_ARGS(&wrapped)));
    }
    bool probe(unsigned* width, unsigned* height, unsigned* format) {
        if (!ReceiveSenderData() || !m_pSharedTexture) return false;
        *width = GetSenderWidth(); *height = GetSenderHeight(); *format = GetSenderFormat();
        IsUpdated();
        return true;
    }
    bool receive(ID3D12Resource* texture) {
        if (!m_pSharedTexture || !bind(texture, D3D12_RESOURCE_STATE_COPY_DEST)) return false;
        if (!frame.CheckTextureAccess(m_pSharedTexture)) return false;
        if (frame.GetNewFrame()) UpdateWrappedResource(wrapped, m_pSharedTexture);
        frame.AllowTextureAccess(m_pSharedTexture);
        return true;
    }
};
#define API extern "C" __declspec(dllexport)
API void* vjay_spout_create(void* device, void* queue, const char* name, bool sender) {
    auto* endpoint = new (std::nothrow) Endpoint;
    if (!endpoint) return nullptr;
    IUnknown* queues[] = {static_cast<ID3D12CommandQueue*>(queue)};
    if (!endpoint->OpenDirectX12(static_cast<ID3D12Device*>(device), queues)) { delete endpoint; return nullptr; }
    if (sender) endpoint->SetSenderName(name); else endpoint->SetReceiverName(name);
    return endpoint;
}
API void vjay_spout_destroy(void* handle) { delete static_cast<Endpoint*>(handle); }
API bool vjay_spout_send(void* handle, void* resource) {
    auto* endpoint = static_cast<Endpoint*>(handle);
    return endpoint->bind(static_cast<ID3D12Resource*>(resource), D3D12_RESOURCE_STATE_COPY_SOURCE) && endpoint->SendDX11Resource(endpoint->wrapped);
}
API bool vjay_spout_probe(void* handle, unsigned* width, unsigned* height, unsigned* format) { return static_cast<Endpoint*>(handle)->probe(width,height,format); }
API bool vjay_spout_receive(void* handle, void* texture) { return static_cast<Endpoint*>(handle)->receive(static_cast<ID3D12Resource*>(texture)); }
API int vjay_spout_source(int index, char* name, int capacity) {
    spoutDX sources;
    int count = sources.GetSenderCount();
    if (index >= 0 && index < count && capacity > 0) sources.GetSender(index, name, capacity);
    return count;
}
