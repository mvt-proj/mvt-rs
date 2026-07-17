# Layout
home = Início
catalog = Catálogo
styles = Estilos
sprites = Sprites
glyphs = Fontes
tutorial = Tutorial
login = Entrar
logout = Sair
password = Senha
groups = Grupos
users = Usuários
categories = Categorias
metadata = Metadados
metrics = Métricas

# Pages
welcome = Bem-vindo ao MVT Server!
welcome-admin = Modo Administrador!
index-subtitle = Sirva camadas do PostGIS como Mapbox Vector Tiles com cache, autenticação e gerenciamento de estilos.
home-capabilities = Recursos
home-cta-catalog = Ver Catálogo
home-cta-styles = Ver Estilos
home-cta-admin = Painel de Administração
feature-1 = Tiles Vetoriais
feature-1-desc = Sirva tabelas e views do PostGIS como tiles .pbf com cache em Redis ou disco.
feature-2 = Servidor de Estilos
feature-2-desc = Publique e compartilhe estilos MapLibre/Mapbox a partir do servidor de estilos integrado.
feature-3 = Controle de Acesso
feature-3-desc = Gerencie usuários e grupos com autenticação JWT e permissões por camada.
feature-4 = Múltiplos Bancos de Dados
feature-4-desc = Conecte vários bancos de dados PostGIS simultaneamente e sirva camadas de cada um de forma independente.
feature-5 = Plugins Lua
feature-5-desc = Injete filtros SQL personalizados em tempo de execução via scripts Lua — por camada ou por categoria, com acesso ao usuário, grupos e nível de zoom.
feature-6 = Metadados (em breve)
feature-6-desc = Servidor de metadados compatível com ISO 19115 para documentação de dados espaciais.

catalog-of-layers = Catálogo de camadas publicadas
list-of-styles = Lista de Estilos
list-of-sprites = Lista de Sprites
list-of-glyps = Lista de Fontes

# Admin
change-password = Alterar Senha

list-of-groups = Lista de Grupos
add-group = ADICIONAR GRUPO
new-group = Novo Grupo
create-group = Criar Grupo
update-group = Atualizar Grupo
edit-group = Editar Grupo
confirm-delete-group = Tem certeza de que deseja excluir este grupo?

list-of-users = Lista de Usuários
add-user = ADICIONAR USUÁRIO
new-user = Novo Usuário
create-user = Criar Usuário
update-user = Atualizar Usuário
edit-user = Editar Usuário
confirm-delete-user = Tem certeza de que deseja excluir este usuário?

list-of-categories = Lista de Categorias
add-category = ADICIONAR CATEGORIA
new-category = Nova Categoria
create-category = Criar Categoria
update-category = Atualizar Categoria
edit-category = Editar Categoria
confirm-delete-category = Tem certeza de que deseja excluir esta categoria?

add-style = ADICIONAR ESTILO
new-style = Novo Estilo
create-style = Criar Estilo
update-style = Atualizar Estilo
edit-style = Editar Estilo
confirm-delete-style = Tem certeza de que deseja excluir este estilo?
apply-style = Aplicar
full-style-button = Inserir exemplo completo
partial-style-button = Inserir exemplo de camada

add-layer = PUBLICAR CAMADA
new-layer = Nova Camada
create-layer = Criar Camada
update-layer = Atualizar Camada
edit-layer = Editar Camada
confirm-delete-cache = Tem certeza de que deseja excluir o cache desta camada?
confirm-delete-layer = Tem certeza de que deseja excluir esta camada?

# Common
filter = Filtro
back = Voltar
help = Ajuda
configuration = Configuração
copy = Copiar
map = Mapa
legends = Legendas
info = Informações
edit = Editar
switch-published = Publicar / Despublicar
delete-cache = Excluir Cache
delete = Excluir
open-json = Abrir JSON
open-png = Abrir PNG

# Maps
zoom-level = Nível de zoom
center = Centro

# Fields
new-password = Nova Senha
email = E-mail
username = Nome de Usuário
first-name = Nome
last-name = Sobrenome
name = Nome
info-name-format = Somente letras minúsculas, números e sublinhado. Espaços viram _ e acentos são removidos automaticamente
layer-name = Nome da Camada
category = Categoria
description = Descrição
style = Estilo
invalid-json = Formato JSON inválido. Verifique a sintaxe.
style-lint-valid = O estilo é válido de acordo com a especificação MapLibre
style-lint-errors = Erros da especificação MapLibre
geometry = Geometria
points = Pontos
lines = Linhas
polygons = Polígonos
alias = Alias
database = Banco de Dados
schema = Esquema
table = Tabela
fields = Campos
sql-mode = Modo SQL
geom = Geom
srid = SRID
buffer = Buffer
extent = Extent
zmin = Zoom mínimo
zmax = Zoom máximo
zmax-change-buffer-extent = Limite de zoom para novo Buffer/Extent
buffer-higher-zoom = Buffer (para zoom mais alto)
extent-higher-zoom = Extent (para zoom mais alto)
clip-geom = Recortar Geometria
delete-cache-on-start = Excluir cache ao iniciar
max-cache-age = Idade máxima do cache
info-time-in-seconds = O tempo deve ser informado em segundos
info-value-infinity = O valor 0 significa infinito
max-records = Número máximo de registros
info-max-records = Máximo de registros a recuperar. Usar 0 ignora esta diretiva.
published = Publicado
allowed-groups = Grupos Permitidos
info-empty-allowed-groups = Se estiver vazio, todos os grupos são permitidos
yes = Sim
no = Não
up = Subir
down = Descer
cancel = Cancelar
no-users-found = Nenhum usuário encontrado
no-groups-found = Nenhum grupo encontrado
no-categories-found = Nenhuma categoria encontrada
plugins = Plugins
no-plugins-found = Nenhum plugin instalado
plugins-dir-hint = Coloque arquivos .lua no diretório de plugins para começar.
plugin-layer = Camada
plugin-category = Categoria
show-code = Ver código
