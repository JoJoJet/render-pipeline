//! A shader that reads a mesh's custom vertex attribute.

use bevy::{
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_graph,
        render_resource::{
            BindGroup, BindGroupDescriptor, BindGroupLayout, BindGroupLayoutDescriptor, BlendState,
            Buffer, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId, ColorTargetState,
            ColorWrites, FragmentState, LoadOp, MultisampleState, Operations, PipelineCache,
            PrimitiveState, PrimitiveTopology, RenderPassDescriptor, RenderPipelineDescriptor,
            TextureFormat, VertexAttribute, VertexBufferLayout, VertexFormat, VertexState,
            VertexStepMode,
        },
        renderer::{RenderContext, RenderDevice},
        texture::BevyDefault,
        view::ViewTarget,
        Render, RenderApp, RenderSet,
    },
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MyRenderPlugin))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, _meshes: ResMut<Assets<Mesh>>) {
    // camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}

struct MyRenderPlugin;

impl Plugin for MyRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MyRender>()
            .add_plugins(ExtractResourcePlugin::<MyRender>::default());

        let render_app = app.sub_app_mut(RenderApp);
        render_app.add_systems(Render, prepare_bind_group.in_set(RenderSet::Prepare));

        let node =MyRenderNode::from_world(&mut render_app.world);
        let mut render_graph = render_app.world.resource_mut::<render_graph::RenderGraph>();
        render_graph.add_node("my_render", node);
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app.init_resource::<MyRenderPipeline>();
    }
}

#[derive(Resource, ExtractResource, Default, Clone)]
struct MyRender {}

#[derive(Resource)]
struct MyRenderPipeline {
    bind_group_layout: BindGroupLayout,
    render_pipeline: CachedRenderPipelineId,
}

impl FromWorld for MyRenderPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: None,
                entries: &[],
            });

        let shader = world.resource::<AssetServer>().load("shader.wgsl");
        let pipeline_cache = world.resource::<PipelineCache>();
        let render_pipeline = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("my_render_pipeline".into()),
            layout: vec![],
            push_constant_ranges: Vec::new(),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                ..default()
            },
            vertex: VertexState {
                shader: shader.clone(),
                entry_point: "vertex".into(),
                shader_defs: vec![],
                buffers: vec![VertexBufferLayout {
                    array_stride: 4 * 2,
                    step_mode: VertexStepMode::Vertex,
                    attributes: vec![VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    }],
                }],
            },
            fragment: Some(FragmentState {
                shader: shader.clone(),
                entry_point: "fragment".into(),
                shader_defs: vec![],
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::bevy_default(),
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        });

        Self {
            bind_group_layout,
            render_pipeline,
        }
    }
}

#[derive(Resource)]
struct MyRenderBindings {
    vertex_buffer: Buffer,
    bind_group: BindGroup,
}

fn prepare_bind_group(
    _splat: Res<MyRender>,
    pipeline: Res<MyRenderPipeline>,
    render_device: Res<RenderDevice>,
    mut commands: Commands,
) {
    let verts = [
        Vec2::new(-1.0, -1.0),
        Vec2::new(1.0, -1.0),
        Vec2::new(1.0, 1.0),
    ];
    let vertex_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&verts),
        usage: BufferUsages::VERTEX,
    });
    let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &pipeline.bind_group_layout,
        entries: &[],
    });
    commands.insert_resource(MyRenderBindings {
        vertex_buffer,
        bind_group,
    });
}

struct MyRenderNode {
    view_target_query: QueryState<&'static ViewTarget>,
}

impl FromWorld for MyRenderNode {
    fn from_world(world: &mut World) -> Self {
        Self {
            view_target_query: QueryState::new(world),
        }
    }
}

impl render_graph::Node for MyRenderNode {
    fn update(&mut self, world: &mut World) {
        self.view_target_query.update_archetypes(world);
    }

    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let MyRenderBindings {
            vertex_buffer,
            bind_group,
        } = world.resource();
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<MyRenderPipeline>();

        let view = {
            let mut views = self.view_target_query.iter_manual(world);
            let v = views.next().unwrap();
            assert!(views.next().is_none());
            v
        };

        let mut pass = render_context
            .command_encoder()
            .begin_render_pass(&RenderPassDescriptor {
                label: Some("my_render_pass"),
                color_attachments: &[Some(view.get_unsampled_color_attachment(Operations {
                    load: LoadOp::Clear(Color::BLACK.into()),
                    store: true,
                }))],
                depth_stencil_attachment: None,
            });

        pass.set_vertex_buffer(0, (*vertex_buffer.slice(..)).clone());
        pass.set_bind_group(0, bind_group, &[]);

        let render_pipeline = pipeline_cache
            .get_render_pipeline(pipeline.render_pipeline)
            .unwrap();
        pass.set_pipeline(render_pipeline);

        pass.draw(0..3, 0..1);

        Ok(())
    }
}
