# Normalización de campos `name` en el admin — Diseño

**Fecha:** 2026-07-11
**Estado:** Aprobado

## Objetivo

Los nombres de capas (catálogo), estilos, categorías y grupos deben quedar siempre
en minúsculas, sin espacios ni caracteres especiales: solo `[a-z0-9_]`.
Ejemplo: `"Departamentos Capital"` → `"departamentos_capital"`,
`"Categoría Ríos"` → `"categoria_rios"`.

Motivación: estos nombres se usan en URLs de tiles y estilos y como claves de
caché (`{categoria}_{capa}`), donde espacios, mayúsculas y acentos causan
problemas (URLs codificadas, claves inconsistentes).

## Alcance

- **Entidades:** capas del catálogo, estilos, categorías, grupos. Usuarios NO.
- **Normalización en dos niveles:** navegador (feedback en vivo) + servidor
  (garantía, cubre también la API JSON `POST /api/catalog/layer`).
- **Datos existentes:** no se migran. La normalización aplica al crear o al
  guardar una edición. Consecuencia asumida: editar y guardar una entidad vieja
  con espacios/mayúsculas la renombra y cambia su URL (la caché se invalida con
  el flujo de edición existente).

## Regla de normalización

Aplicada de forma idéntica en Rust y JS:

1. `trim()` de los extremos
2. Pasar a minúsculas
3. Transliterar acentos y eñes: `á é í ó ú ü ñ` → `a e i o u u n`
4. Espacios y tabs (secuencias completas) → `_`
5. Descartar todo carácter que no sea `[a-z0-9_]`
6. Colapsar `_` repetidos

Si el resultado queda vacío (ej. entrada `"!!!"`), el guardado falla con error
de validación.

## Componentes

### 1. `normalize_name()` en Rust — `src/services/utils.rs`

Función pública compartida junto a los helpers existentes (`convert_fields`,
`validate_filter`). Con tests unitarios (mayúsculas, acentos, espacios
múltiples, símbolos, resultado vacío).

### 2. Aplicación en el servidor — en los modelos

Un solo punto por entidad, cubre formularios HTML y API JSON:

| Entidad | Métodos |
|---|---|
| Grupo | `Group::new`, `update_group` (`src/auth/models.rs`) |
| Categoría | `Category::new`, `update_category` (`src/models/category.rs`) |
| Estilo | `Style::new`, `update_style` (`src/models/styles.rs`) |
| Capa | `Catalog::add_layer`, `update_layer` (`src/models/catalog.rs`) |

### 3. JS en el navegador — `templates/admin/layout_admin.html`

En el bloque `<script>` compartido del layout del admin: listener de `input`
sobre todo campo con el atributo `data-normalize-name`, que reemplaza el valor
por su versión normalizada mientras se escribe, preservando la posición del
cursor.

Los 8 templates (`new`/`edit` × 4 entidades) agregan a su input `name`:

- `data-normalize-name`
- `pattern="[a-z0-9_]+"` como respaldo sin JS

## Errores

Nombre vacío tras normalizar → el modelo devuelve `AppError` de validación; los
handlers existentes ya traducen errores a HTML o JSON según `Accept`.

## Testing

- Tests unitarios de `normalize_name()` en Rust.
- Verificación manual de un formulario (crear categoría con
  `"Categoría  De Prueba"` → se guarda `categoria_de_prueba`).
