use {ResourceIndex, BufferPtr, SamplerPtr, TexturePtr};
use command::IndexBuffer;
use native::RasterizerState;

use hal;
use metal;

use std::ops::Range;


pub type CacheResourceIndex = u32;

pub trait Resources {
    type Data;
    type BufferArray;
    type DepthStencil;
    type RenderPipeline;
    type ComputePipeline;
}

#[derive(Debug, Default)]
pub struct Own {
    pub buffers: Vec<Option<BufferPtr>>,
    pub buffer_offsets: Vec<hal::buffer::Offset>,
    pub textures: Vec<Option<TexturePtr>>,
    pub samplers: Vec<Option<SamplerPtr>>,
}

impl Resources for Own {
    type Data = Vec<u32>;
    type BufferArray = Range<CacheResourceIndex>;
    type DepthStencil = metal::DepthStencilState;
    type RenderPipeline = metal::RenderPipelineState;
    type ComputePipeline = metal::ComputePipelineState;
}

#[derive(Debug)]
pub struct Ref;
impl<'a> Resources for &'a Ref {
    type Data = &'a [u32];
    type BufferArray = (&'a [Option<BufferPtr>], &'a [hal::buffer::Offset]);
    type DepthStencil = &'a metal::DepthStencilStateRef;
    type RenderPipeline = &'a metal::RenderPipelineStateRef;
    type ComputePipeline = &'a metal::ComputePipelineStateRef;
}

#[derive(Clone, Debug)]
pub enum RenderCommand<R: Resources> {
    SetViewport(hal::pso::Rect, Range<f32>),
    SetScissor(metal::MTLScissorRect),
    SetBlendColor(hal::pso::ColorValue),
    SetDepthBias(hal::pso::DepthBias),
    SetDepthStencilState(R::DepthStencil),
    SetStencilReferenceValues(hal::pso::StencilValue, hal::pso::StencilValue),
    SetRasterizerState(RasterizerState),
    BindBuffer {
        stage: hal::pso::Stage,
        index: ResourceIndex,
        buffer: Option<(BufferPtr, hal::buffer::Offset)>,
    },
    BindBuffers {
        stage: hal::pso::Stage,
        index: ResourceIndex,
        buffers: R::BufferArray,
    },
    BindBufferData {
        stage: hal::pso::Stage,
        index: ResourceIndex,
        words: R::Data,
    },
    BindTexture {
        stage: hal::pso::Stage,
        index: ResourceIndex,
        texture: Option<TexturePtr>,
    },
    BindSampler {
        stage: hal::pso::Stage,
        index: ResourceIndex,
        sampler: Option<SamplerPtr>,
    },
    BindPipeline(R::RenderPipeline),
    Draw {
        primitive_type: metal::MTLPrimitiveType,
        vertices: Range<hal::VertexCount>,
        instances: Range<hal::InstanceCount>
    },
    DrawIndexed {
        primitive_type: metal::MTLPrimitiveType,
        index: IndexBuffer<BufferPtr>,
        indices: Range<hal::IndexCount>,
        base_vertex: hal::VertexOffset,
        instances: Range<hal::InstanceCount>,
    },
    DrawIndirect {
        primitive_type: metal::MTLPrimitiveType,
        buffer: BufferPtr,
        offset: hal::buffer::Offset,
    },
    DrawIndexedIndirect {
        primitive_type: metal::MTLPrimitiveType,
        index: IndexBuffer<BufferPtr>,
        buffer: BufferPtr,
        offset: hal::buffer::Offset,
    },
}

#[derive(Clone, Debug)]
pub enum BlitCommand {
    CopyBuffer {
        src: BufferPtr,
        dst: BufferPtr,
        region: hal::command::BufferCopy,
    },
    CopyImage {
        src: TexturePtr,
        dst: TexturePtr,
        region: hal::command::ImageCopy,
    },
    CopyBufferToImage {
        src: BufferPtr,
        dst: TexturePtr,
        dst_desc: hal::format::FormatDesc,
        region: hal::command::BufferImageCopy,
    },
    CopyImageToBuffer {
        src: TexturePtr,
        src_desc: hal::format::FormatDesc,
        dst: BufferPtr,
        region: hal::command::BufferImageCopy,
    },
}

#[derive(Clone, Debug)]
pub enum ComputeCommand<R: Resources> {
    BindBuffer {
        index: ResourceIndex,
        buffer: Option<(BufferPtr, hal::buffer::Offset)>,
    },
    BindBuffers {
        index: ResourceIndex,
        buffers: R::BufferArray,
    },
    BindBufferData {
        index: ResourceIndex,
        words: R::Data,
    },
    BindTexture {
        index: ResourceIndex,
        texture: Option<TexturePtr>,
    },
    BindSampler {
        index: ResourceIndex,
        sampler: Option<SamplerPtr>,
    },
    BindPipeline(R::ComputePipeline),
    Dispatch {
        wg_size: metal::MTLSize,
        wg_count: metal::MTLSize,
    },
    DispatchIndirect {
        wg_size: metal::MTLSize,
        buffer: BufferPtr,
        offset: hal::buffer::Offset,
    },
}


#[derive(Debug)]
pub enum Pass {
    Render(metal::RenderPassDescriptor),
    Blit,
    Compute,
}

impl Own {
    pub fn clear(&mut self) {
        self.buffers.clear();
        self.buffer_offsets.clear();
        self.textures.clear();
        self.samplers.clear();
    }

    pub fn own_render(&mut self, com: RenderCommand<&Ref>) -> RenderCommand<Self> {
        use self::RenderCommand::*;
        match com {
            SetViewport(rect, depth) => SetViewport(rect, depth),
            SetScissor(rect) => SetScissor(rect),
            SetBlendColor(color) => SetBlendColor(color),
            SetDepthBias(bias) => SetDepthBias(bias),
            SetDepthStencilState(state) => SetDepthStencilState(state.to_owned()),
            SetStencilReferenceValues(front, back) => SetStencilReferenceValues(front, back),
            SetRasterizerState(ref state) => SetRasterizerState(state.clone()),
            BindBuffer { stage, index, buffer } => BindBuffer {
                stage,
                index,
                buffer,
            },
            BindBuffers { stage, index, buffers: (buffers, offsets) } => BindBuffers {
                stage,
                index,
                buffers: {
                    let buf_start = self.buffers.len() as CacheResourceIndex;
                    self.buffers.extend_from_slice(buffers);
                    self.buffer_offsets.extend_from_slice(offsets);
                    buf_start .. self.buffers.len() as CacheResourceIndex
                },
            },
            BindBufferData { stage, index, words } => BindBufferData {
                stage,
                index,
                words: words.to_vec(),
            },
            BindTexture { stage, index, texture } => BindTexture {
                stage,
                index,
                texture,
            },
            BindSampler { stage, index, sampler } => BindSampler {
                stage,
                index,
                sampler,
            },
            BindPipeline(pso) => BindPipeline(pso.to_owned()),
            Draw { primitive_type, vertices, instances } => Draw {
                primitive_type,
                vertices,
                instances,
            },
            DrawIndexed { primitive_type, index, indices, base_vertex, instances } => DrawIndexed {
                primitive_type,
                index,
                indices,
                base_vertex,
                instances,
            },
            DrawIndirect { primitive_type, buffer, offset } => DrawIndirect {
                primitive_type,
                buffer,
                offset,
            },
            DrawIndexedIndirect { primitive_type, index, buffer, offset } => DrawIndexedIndirect {
                primitive_type,
                index,
                buffer,
                offset,
            },
        }
    }

    pub fn own_compute(&mut self, com: ComputeCommand<&Ref>) -> ComputeCommand<Self> {
        use self::ComputeCommand::*;
        match com {
            BindBuffer { index, buffer } => BindBuffer {
                index,
                buffer,
            },
            BindBuffers { index, buffers: (buffers, offsets) } => BindBuffers {
                index,
                buffers: {
                    let buf_start = self.buffers.len() as CacheResourceIndex;
                    self.buffers.extend_from_slice(buffers);
                    self.buffer_offsets.extend_from_slice(offsets);
                    buf_start .. self.buffers.len() as CacheResourceIndex
                },
            },
            BindBufferData { index, words } => BindBufferData {
                index,
                words: words.to_vec(),
            },
            BindTexture { index, texture } => BindTexture {
                index,
                texture,
            },
            BindSampler { index, sampler } => BindSampler {
                index,
                sampler,
            },
            BindPipeline(pso) => BindPipeline(pso.to_owned()),
            Dispatch { wg_size, wg_count } => Dispatch {
                wg_size,
                wg_count,
            },
            DispatchIndirect { wg_size, buffer, offset } => DispatchIndirect {
                wg_size,
                buffer,
                offset,
            },
        }
    }
}

/// This is a helper trait that allows us to unify owned and non-owned handling
/// of the context-dependent data, such as resource arrays.
pub trait AsSlice<T, R> {
    fn as_slice<'a>(&'a self, resources: &'a R) -> &'a [T];
}
impl<'b, T> AsSlice<Option<T>, &'b Ref> for [Option<T>] {
    #[inline(always)]
    fn as_slice<'a>(&'a self, _: &'a &'b Ref) -> &'a [Option<T>] {
        self
    }
}
impl<'b> AsSlice<Option<BufferPtr>, &'b Ref> for (&'b [Option<BufferPtr>], &'b [hal::buffer::Offset]) {
    #[inline(always)]
    fn as_slice<'a>(&'a self, _: &'a &'b Ref) -> &'a [Option<BufferPtr>] {
        self.0
    }
}
impl<'b> AsSlice<hal::buffer::Offset, &'b Ref> for (&'b [Option<BufferPtr>], &'b [hal::buffer::Offset]) {
    #[inline(always)]
    fn as_slice<'a>(&'a self, _: &'a &'b Ref) -> &'a [hal::buffer::Offset] {
        self.1
    }
}
impl AsSlice<Option<BufferPtr>, Own> for Range<CacheResourceIndex> {
    #[inline(always)]
    fn as_slice<'a>(&'a self, resources: &'a Own) -> &'a [Option<BufferPtr>] {
        &resources.buffers[self.start as usize .. self.end as usize]
    }
}
impl AsSlice<hal::buffer::Offset, Own> for Range<CacheResourceIndex> {
    #[inline(always)]
    fn as_slice<'a>(&'a self, resources: &'a Own) -> &'a [hal::buffer::Offset] {
        &resources.buffer_offsets[self.start as usize .. self.end as usize]
    }
}
impl AsSlice<Option<TexturePtr>, Own> for Range<CacheResourceIndex> {
    #[inline(always)]
    fn as_slice<'a>(&'a self, resources: &'a Own) -> &'a [Option<TexturePtr>] {
        &resources.textures[self.start as usize .. self.end as usize]
    }
}
impl AsSlice<Option<SamplerPtr>, Own> for Range<CacheResourceIndex> {
    #[inline(always)]
    fn as_slice<'a>(&'a self, resources: &'a Own) -> &'a [Option<SamplerPtr>] {
        &resources.samplers[self.start as usize .. self.end as usize]
    }
}


fn _test_render_command_size(com: RenderCommand<Own>) -> [usize; 6] {
    use std::mem;
    unsafe { mem::transmute(com) }
}
