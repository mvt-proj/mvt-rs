# Tutorial

mvt-server allows you to publish geographic layers in vector tile format on an intranet or the internet for consumption by desktop clients like QGIS, or web clients such as MapLibre, OpenLayers, or Leaflet.

mvt-server not only allows you to publish geographic layers in vector tile format, but also includes an administration panel that simplifies the management of publishing your layers and configuring styles.

To access the mvt-server administration interface, simply enter the address http://localhost:5800 (or the corresponding domain if it is hosted on a remote server) in your web browser. Once there, you can manage your geographic layers, styles, and other server configurations.

![imagen](https://github.com/user-attachments/assets/82a1d638-83c9-4a3d-b92a-1c1c5911d9f8)

The initial access credentials for mvt-server are: email **admin@mail.com** and password **admin**. It is of utmost importance that, upon your first access to the platform, you change this default password to a new, strong password of your choice. This will help protect your server and data from unauthorized access

![imagen](https://github.com/user-attachments/assets/2ce993cd-5bc3-42c4-be23-311bca4bbd7c)

### mvt-server Administration Panel

The mvt-server administration panel is an essential tool for managing all aspects of publishing your geographic layers as vector tiles. Through an intuitive web interface, you'll have control over:

#### 1. Groups (User Roles)

    Creation and Management: Define user groups or roles with different levels of access and permissions. This allows you to control who can perform administrative tasks, publish layers, create styles, etc.    
    Permission Assignment: Assign specific permissions to each group to granularly control access to the server's various functionalities.

#### 2. Users

    Creation and Management: Create new user accounts and manage existing ones.
    Role Assignment: Assign users to specific groups to determine their permissions and level of access.
    Administrative Users: Only users belonging to the "admin" group (or another that is configured as such) will have the ability to perform administrative tasks, such as managing users, groups, categories, catalog, and styles.

#### 3. Categories

    Logical Organization: Categories act as namespaces to organize your layers and styles logically. This is especially useful when working with a large number of layers, as it allows you to keep them organized and easy to find.
    

#### 4. Catalog (Layer Publishing)

    Layer Declaration: This is the central section of the administrator. Here you define and declare the geographic layers you want to publish as vector tiles.
    Layer Configuration: Specify the data source for each layer, projections, and other relevant parameters.
    Publishing: Once configured, the layers will be available for publishing as vector tiles.

#### 5. Styles

    Creation and Management: Define and manage rendering styles for your layers. Styles determine how the layers will appear on the map (colors, symbols, labels, etc.).
    Style Publishing: Publish the created styles so they can be used by clients like QGIS.
    Style Language: mvt-server likely supports a style language like the Maplibre Style Specification, which allows you to define complex and custom styles.

#### In summary

The mvt-server administration panel gives you complete control over the publication of your geographic layers as vector tiles. From managing users and permissions to the detailed configuration of layers and styles, this tool allows you to create and maintain interactive and personalized maps efficiently.

![imagen](https://github.com/user-attachments/assets/accf44c6-644f-48fd-933a-9b2f65b2dd59)


